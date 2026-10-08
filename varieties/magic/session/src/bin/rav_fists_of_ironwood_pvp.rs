//! Full-game diagnostic PvP authority for RAV Fists of Ironwood.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, ContinuousChange, CreatureSubtype, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FISTS-OF-IRONWOOD";
const TRIGGER_ID: &str = "etb-two-saprolings";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const BLOCKER_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/fists_of_ironwood.rs";
const MAX_TURNS: u32 = 50;
const MAX_POLICY_MOVES: u32 = 8_000;
const REFERENCE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/fists_of_ironwood.rs"]
mod candidate_fists_of_ironwood;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Fists of Ironwood result: {error}");
                return ExitCode::FAILURE;
            }
            println!(
                "{}",
                serde_json::to_string(&report).expect("report serializes")
            );
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Fists of Ironwood PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let reference = run_match(REFERENCE_SEED, Mutation::None)?;
    let trace = validate_trace(&reference)?;
    let replay = run_match(REFERENCE_SEED, Mutation::None)?;
    if !same_match(&reference, &replay) {
        return Err("Fists of Ironwood full-game replay was not deterministic".to_owned());
    }
    let no_trigger = mutation_detected(Mutation::NoTrigger)?;
    let no_trample = mutation_detected(Mutation::NoTrample)?;
    let wrong_token_count = mutation_detected(Mutation::WrongTokenCount)?;
    if !no_trigger || !no_trample || !wrong_token_count {
        return Err("a Fists of Ironwood semantic mutation escaped the gameplay oracle".to_owned());
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 164, "definition_id": CARD_ID, "name": "Fists of Ironwood"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "303.4: Aura enchant restrictions and attachment",
                "302.1 and 601.2: permanent spell casting and resolution",
                "603.2 and 603.3: mandatory enter-the-battlefield triggered abilities",
                "608.2: resolving triggered abilities",
                "110 and 111: token creation and characteristics",
                "702.19: Trample",
                "510.1-510.2: combat damage assignment and resolution"
            ],
            "source_contract": "RAV Fists of Ironwood is a green {1}{G} Enchantment Aura for a Creature; while attached it grants Trample, and its mandatory ETB trigger creates exactly two green 1/1 Saproling creatures",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": {
            "etb_trigger_removed_mutation_detected_in_game": no_trigger,
            "trample_attachment_effect_removed_mutation_detected_in_game": no_trample,
            "saproling_count_changed_mutation_detected_in_game": wrong_token_count,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoTrigger,
    NoTrample,
    WrongTokenCount,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    fists: Option<ObjectId>,
    target: Option<ObjectId>,
    blocker: Option<ObjectId>,
    trigger_incarnation: Option<u64>,
    attached_to: Option<ObjectId>,
    fists_zone: Option<Zone>,
    target_has_trample: bool,
    token_objects: Vec<ObjectId>,
    token_specs_valid: Vec<bool>,
    target_damage_to_blocker: i32,
    target_damage_to_player: i32,
    block_was_declared: bool,
}

fn mutation_detected(mutation: Mutation) -> Result<bool, String> {
    match run_match(REFERENCE_SEED, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
    }
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left == right
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Fists of Ironwood definition is missing from the RAV registry")?;
    *candidate = candidate_fists_of_ironwood::definition();
    if mutation == Mutation::NoTrample {
        candidate.effects = vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }];
    }

    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Fists of Ironwood ETB trigger is missing from the RAV registry")?;
    triggers[trigger_index] = candidate_fists_of_ironwood::triggered_ability();
    match mutation {
        Mutation::None | Mutation::NoTrample => {}
        Mutation::NoTrigger => {
            triggers.remove(trigger_index);
        }
        Mutation::WrongTokenCount => {
            triggers[trigger_index].ability.effects = vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }];
        }
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;

    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("Fists task deck", &p0_deck),
        ("blocker task deck", &p1_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FistsPolicy::new(PlayerId(0))),
        Box::new(FistsPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut fists = None;
    let mut target = None;
    let mut blocker = None;
    let mut trigger_incarnation = None;
    let mut token_objects = Vec::new();
    let mut target_has_trample = false;
    let mut block_was_declared = false;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!(
                "Fists of Ironwood duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat is unavailable")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        if let PolicyAction::DeclareBlockers { assignments } = &action
            && player == PlayerId(1)
            && !assignments.is_empty()
        {
            if assignments.len() != 1 {
                return Err(
                    "blocker policy submitted more than the single trample audit block".to_owned(),
                );
            }
            blocker = assignments.first().map(|assignment| assignment.blocker);
            block_was_declared = true;
        }
        let old_event_count = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Fists policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    fists = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    target = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == BLOCKER_ID) =>
                {
                    blocker = Some(*card)
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                    ..
                } if Some(*source) == fists => trigger_incarnation = Some(*source_incarnation),
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token,
                } if trigger_incarnation.is_some() => {
                    if !token_objects.contains(token) {
                        token_objects.push(*token);
                    }
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if Some(target.unwrap_or(ObjectId(u64::MAX)))
                    .is_some_and(|target| attackers.contains(&target)) =>
                {
                    if let Some(target) = target {
                        target_has_trample =
                            game.characteristics(target).is_ok_and(|characteristics| {
                                characteristics.keywords.contains(&Keyword::Trample)
                            });
                    }
                }
                _ => {}
            }
        }
    }

    let fists_object = fists;
    let target_object = target;
    let attached_to = fists_object.and_then(|source| {
        game.object(source)
            .ok()
            .and_then(|object| object.attached_to)
    });
    let fists_zone = fists_object.and_then(|source| game.zone_of(source));
    let token_specs_valid = token_objects
        .iter()
        .map(|token| {
            let spec_is_correct = game
                .object(*token)
                .is_ok_and(|object| object.token.as_ref() == Some(&TokenSpec::saproling()));
            let characteristics_are_correct =
                game.characteristics(*token).is_ok_and(|characteristics| {
                    characteristics.colors == BTreeSet::from([Color::Green])
                        && characteristics.card_types.contains(&CardType::Creature)
                        && characteristics
                            .creature_subtypes
                            .contains(&CreatureSubtype::Saproling)
                        && characteristics.power == Some(1)
                        && characteristics.toughness == Some(1)
                        && characteristics.keywords.is_empty()
                });
            spec_is_correct
                && characteristics_are_correct
                && game.controller_of(*token).ok() == Some(PlayerId(0))
                && game.zone_of(*token) == Some(Zone::Battlefield)
        })
        .collect();
    let target_damage_to_blocker: i32 = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if Some(*source) == target_object && Some(*permanent) == blocker => Some(*amount),
            _ => None,
        })
        .sum();
    let target_damage_to_player: i32 = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == target_object => Some(*amount),
            _ => None,
        })
        .sum();
    let winner = game.winner().map(|winner| winner.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        fists: fists_object,
        target: target_object,
        blocker,
        trigger_incarnation,
        attached_to,
        fists_zone,
        target_has_trample,
        token_objects,
        token_specs_valid,
        target_damage_to_blocker,
        target_damage_to_player,
        block_was_declared,
    })
}

struct FistsPolicy {
    player: PlayerId,
    fists_cast: bool,
    watchwolf_cast: bool,
    blocker_cast: bool,
    trample_block_used: bool,
}

impl FistsPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            fists_cast: false,
            watchwolf_cast: false,
            blocker_cast: false,
            trample_block_used: false,
        }
    }

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(candidate, _)| candidate == color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
            generic.push(*color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && preferred.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = preferred
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn cast(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(definition))?;
            match definition {
                CARD_ID => self.fists_cast = true,
                WATCHWOLF_ID => self.watchwolf_cast = true,
                BLOCKER_ID => self.blocker_cast = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(Color::Green)
            });
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for FistsPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.fists-of-ironwood-aura-trample-tokens.v1"
        } else {
            "rav.fists-of-ironwood-single-trample-block.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
                    .map(|card| card.id)
                    .collect()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player == PlayerId(1) && !self.trample_block_used {
                let attacker = view.combat_attackers.iter().find(|card| {
                    card.definition == Some(WATCHWOLF_ID)
                        && card.keywords.contains(&Keyword::Trample)
                });
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.trample_block_used = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            if !self.watchwolf_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
            if !self.fists_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                && let Some(target) = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(WATCHWOLF_ID)
                        && card.card_types.contains(&CardType::Creature)
                })
            {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    ManaCost::with_colors(1, [Color::Green]),
                    vec![Target::Permanent(target.id)],
                ) {
                    return action;
                }
            }
        } else if !self.blocker_cast
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
        {
            if let Some(action) = self.cast(
                view,
                BLOCKER_ID,
                ManaCost::with_colors(0, [Color::Green]),
                vec![],
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Fists of Ironwood duel did not end in a clean P0 win".to_owned());
    }
    let fists = capture.fists.ok_or("Fists of Ironwood was not cast")?;
    let target = capture.target.ok_or("Watchwolf target was not cast")?;
    let blocker = capture
        .blocker
        .ok_or("Elves of Deep Shadow blocker was not cast")?;
    if capture.fists_zone != Some(Zone::Battlefield) || capture.attached_to != Some(target) {
        return Err(
            "Fists of Ironwood did not remain attached to its exact Watchwolf target".to_owned(),
        );
    }
    if !capture.target_has_trample {
        return Err(
            "the exact attached Watchwolf did not have live Trample at attacker declaration"
                .to_owned(),
        );
    }
    if capture.token_objects.len() != 2 || capture.token_specs_valid != [true, true] {
        return Err(format!(
            "ETB ability must create exactly two live green 1/1 Saprolings: found {:?}",
            capture.token_specs_valid
        ));
    }
    if !capture.block_was_declared
        || capture.target_damage_to_blocker != 1
        || capture.target_damage_to_player < 1
    {
        return Err(format!(
            "Trample must assign lethal damage to the Elf and excess damage to the defending player (blocker {}, player {}; block {})",
            capture.target_damage_to_blocker,
            capture.target_damage_to_player,
            capture.block_was_declared,
        ));
    }

    let payment = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == fists && colors.len() == 2 && colors.contains(&Color::Green))).ok_or("Fists exact {1}{G} spell payment receipt missing")?;
    let cast = event_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == fists)).ok_or("Fists cast receipt missing")?;
    let resolved = event_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == fists),
    )
    .ok_or("Fists spell resolution receipt missing")?;
    let enters = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == fists)).ok_or("Fists Aura battlefield entry receipt missing")?;
    let attached = event_index(&capture.events, |event| matches!(event, GameEvent::AuraAttached { aura, target: attached_target } if *aura == fists && *attached_target == target)).ok_or("exact AuraAttached receipt missing")?;
    let trigger = event_index(&capture.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == fists)).ok_or("Fists mandatory ETB trigger did not stack")?;
    let trigger_incarnation = match capture.events.get(trigger) {
        Some(GameEvent::TriggeredAbilityStacked {
            source_incarnation, ..
        }) => *source_incarnation,
        _ => return Err("Fists trigger incarnation receipt disappeared".to_owned()),
    };
    let token_events: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } if capture.token_objects.contains(token) => Some((index, *token)),
            _ => None,
        })
        .collect();
    if token_events.len() != 2 {
        return Err("Fists ETB must emit exactly two TokenCreated receipts".to_owned());
    }
    let ability = event_index_after(&capture.events, trigger, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: TRIGGER_ID } if *source == fists && *source_incarnation == trigger_incarnation)).ok_or("Fists token trigger did not resolve from the exact Aura incarnation")?;
    let attack = event_index(&capture.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))).ok_or("Trample-granted Watchwolf did not attack")?;
    let blockers = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(target, blocker)))).ok_or("Elf blocker did not block the Trample attacker")?;
    let creature_damage = event_index_after(&capture.events, blockers, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == target && *permanent == blocker)).ok_or("Watchwolf did not assign lethal combat damage to its blocker")?;
    let player_damage = event_index_after(&capture.events, creature_damage, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == target)).ok_or("Watchwolf's exact 2 damage of Trample excess did not reach the defending player")?;
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(payment < cast
        && cast < resolved
        && resolved < enters
        && enters < attached
        && attached < trigger
        && trigger < token_events[0].0
        && token_events[0].0 < token_events[1].0
        && token_events[1].0 < ability
        && ability < attack
        && attack < blockers
        && blockers < creature_damage
        && creature_damage < player_damage
        && player_damage < end)
    {
        return Err("Fists cast, attachment, ETB trigger, token, Trample combat, and terminal receipt order differs".to_owned());
    }

    let mut indices = vec![
        payment,
        cast,
        resolved,
        enters,
        attached,
        trigger,
        token_events[0].0,
        token_events[1].0,
        ability,
        attack,
        blockers,
        creature_damage,
        player_damage,
        end,
    ];
    indices.sort_unstable();
    let events = project_selected_events(&capture.events, &indices);
    Ok(json!({
        "id": "rav-fists-of-ironwood-trample-and-etb-two-saprolings-full-game",
        "seed": REFERENCE_SEED,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "fists_object": fists.0,
        "fists_incarnation_at_trigger": trigger_incarnation,
        "target_object": target.0,
        "target_definition": WATCHWOLF_ID,
        "aura_attached_to_exact_target": capture.attached_to == Some(target),
        "target_has_trample_at_attack": capture.target_has_trample,
        "blocker_object": blocker.0,
        "block_was_declared": capture.block_was_declared,
        "target_damage_to_blocker": capture.target_damage_to_blocker,
        "target_trample_damage_to_player": capture.target_damage_to_player,
        "saproling_objects": capture.token_objects.iter().map(|token| token.0).collect::<Vec<_>>(),
        "saprolings_are_exact_green_1_1_creatures": capture.token_specs_valid,
        "events_digest": event_digest(&capture.events),
        "events": events,
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Fists of Ironwood",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-enchant-creature-trample-etb-two-saprolings",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::Trample)],
        }],
    }
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        },
    }
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_fists_of_ironwood::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Fists of Ironwood is missing from the RAV catalog")?;
    if definition != expected_definition() || definition != registered {
        return Err("Fists of Ironwood literal is outside the task source contract".to_owned());
    }
    let trigger = candidate_fists_of_ironwood::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Fists of Ironwood trigger is missing from the RAV registry")?;
    if trigger != expected_trigger() || trigger != registered {
        return Err(
            "Fists of Ironwood trigger literal is outside the task source contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Fists of Ironwood source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Fists of Ironwood source is not declarative Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TokenSpec"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Fists of Ironwood source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                }) && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.sig.constness.is_none()
                    && item.sig.asyncness.is_none()
                    && item.sig.unsafety.is_none()
                    && item.sig.abi.is_none()
                    && item.sig.variadic.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let (expected, expected_type) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("CardDefinition", "CardDefinition"),
                    "triggered_ability" => ("TriggeredAbilityBinding", "TriggeredAbilityBinding"),
                    _ => {
                        return Err(
                            "Fists of Ironwood source has an unsupported function".to_owned()
                        );
                    }
                };
                let direct = matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, expected));
                if !matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_type))
                    || !direct
                    || !functions.insert(item.sig.ident.to_string())
                {
                    return Err(
                        "Fists of Ironwood functions must return direct typed literals".to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Fists of Ironwood source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Fists of Ironwood source module structure differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type_path(&path.path, name))
}
fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == name
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from the task source contract".to_owned());
    }
    Ok(catalog)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(position, event)| predicate(event).then_some(position))
}
fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<Value> {
    let projected = project_events(events);
    indices.iter().filter_map(|index| projected.get(*index)).map(|event: &ProjectedEvent| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})).collect()
}
fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 164;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}
