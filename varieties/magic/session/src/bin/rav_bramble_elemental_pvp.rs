//! Full-game PvP authority for RAV Bramble Elemental.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
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
const CARD_ID: &str = "RAV-BRAMBLE-ELEMENTAL";
const TRIGGER_ID: &str = "aura-attached-create-two-saprolings";
const AURA_ID: &str = "RAV-MOLDERVINE-CLOAK";
const OTHER_CREATURE_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/bramble_elemental.rs";
const MAX_TURNS: u32 = 50;
const MAX_POLICY_MOVES: u32 = 8_000;
const REFERENCE_SEED: u64 = 0;
const MAX_REFERENCE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/bramble_elemental.rs"]
mod candidate_bramble_elemental;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Bramble Elemental result: {error}");
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
            eprintln!("Bramble Elemental PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let seed = reference_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Bramble Elemental full-game replay was not deterministic".to_owned());
    }
    let no_trigger = mutation_detected(seed, Mutation::NoTrigger)?;
    let wrong_event = mutation_detected(seed, Mutation::WrongAttachmentEvent)?;
    let wrong_count = mutation_detected(seed, Mutation::WrongTokenCount)?;
    let entry_instead_of_attachment = mutation_detected(seed, Mutation::EntersBattlefield)?;
    if !no_trigger || !wrong_event || !wrong_count || !entry_instead_of_attachment {
        return Err("a Bramble Elemental semantic mutation escaped the gameplay oracle".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 154, "definition_id": CARD_ID, "name": "Bramble Elemental"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "303.4: Aura attachment and enchant restrictions",
                "302.1 and 601.2: creature characteristics and spell casting",
                "603.2 and 603.3: triggered event capture and placement",
                "608.2: resolving triggered abilities",
                "110 and 111: token creation and characteristics",
                "510.1-510.2: combat damage and game progression"
            ],
            "source_contract": "RAV Bramble Elemental is a green 4/4 creature costing {3}{G}{G}; when an Aura becomes attached to that exact permanent, its controller may create two green 1/1 Saproling creature tokens",
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
            "trigger_removed_mutation_detected_in_game": no_trigger,
            "controller_wide_aura_entry_instead_of_exact_attachment_detected_in_game": wrong_event,
            "wrong_saproling_count_mutation_detected_in_game": wrong_count,
            "bramble_etb_instead_of_aura_attachment_detected_in_game": entry_instead_of_attachment,
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
    WrongAttachmentEvent,
    WrongTokenCount,
    EntersBattlefield,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    bramble: Option<ObjectId>,
    other_creature: Option<ObjectId>,
    bramble_zone: Option<Zone>,
    aura_objects: Vec<ObjectId>,
    aura_targets: Vec<(ObjectId, ObjectId)>,
    trigger_incarnations: Vec<u64>,
    token_objects: Vec<ObjectId>,
    token_specs_valid: Vec<bool>,
    token_attack_seen: bool,
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let bramble_definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Bramble Elemental definition is missing from the RAV registry")?;
    *bramble_definition = candidate_bramble_elemental::definition();

    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Bramble Elemental attachment trigger is missing from the RAV registry")?;
    triggers[trigger_index] = candidate_bramble_elemental::triggered_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoTrigger => {
            triggers.remove(trigger_index);
        }
        Mutation::WrongAttachmentEvent => {
            triggers[trigger_index].ability.condition =
                TriggerCondition::ControlledAuraEntersBattlefield;
        }
        Mutation::WrongTokenCount => {
            triggers[trigger_index].ability.effects = vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }];
        }
        Mutation::EntersBattlefield => {
            triggers[trigger_index].ability.condition = TriggerCondition::EntersBattlefield;
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
                card: OTHER_CREATURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: AURA_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 20,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 30,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("Bramble task deck", &p0_deck),
        ("opponent task deck", &p1_deck),
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
        Box::new(BramblePolicy::new(PlayerId(0))),
        Box::new(BramblePolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut bramble = None;
    let mut other_creature = None;
    let mut aura_objects = Vec::new();
    let mut aura_targets = Vec::new();
    let mut trigger_incarnations = Vec::new();
    let mut token_objects = Vec::new();
    let mut token_attack_seen = false;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!(
                "Bramble duel exceeded its bound at turn {}",
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
        let old_event_count = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Bramble policy move rejected at turn {} {:?}: {error}",
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
                    bramble = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == OTHER_CREATURE_ID) =>
                {
                    other_creature = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == AURA_ID) =>
                {
                    aura_objects.push(*card)
                }
                GameEvent::AuraAttached { aura, target } if aura_objects.contains(aura) => {
                    aura_targets.push((*aura, *target))
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                    ..
                } if Some(*source) == bramble => trigger_incarnations.push(*source_incarnation),
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token,
                } => {
                    if !token_objects.contains(token) {
                        token_objects.push(*token);
                    }
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if token_objects.len() == 2
                    && token_objects.iter().all(|token| attackers.contains(token)) =>
                {
                    token_attack_seen = true
                }
                _ => {}
            }
        }
    }
    let token_specs_valid = token_objects
        .iter()
        .map(|token| {
            let exact_spec = game
                .object(*token)
                .is_ok_and(|object| object.token.as_ref() == Some(&TokenSpec::saproling()));
            let exact_characteristics = game.characteristics(*token).is_ok_and(|characteristics| {
                characteristics.colors == BTreeSet::from([Color::Green])
                    && characteristics.card_types.contains(&CardType::Creature)
                    && characteristics
                        .creature_subtypes
                        .contains(&cardbench_magic_engine::CreatureSubtype::Saproling)
                    && characteristics.power == Some(1)
                    && characteristics.toughness == Some(1)
                    && characteristics.keywords.is_empty()
            });
            exact_spec
                && exact_characteristics
                && game.controller_of(*token).ok() == Some(PlayerId(0))
                && game.zone_of(*token) == Some(Zone::Battlefield)
        })
        .collect();
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
        bramble,
        other_creature,
        bramble_zone: bramble.and_then(|card| game.zone_of(card)),
        aura_objects,
        aura_targets,
        trigger_incarnations,
        token_objects,
        token_specs_valid,
        token_attack_seen,
    })
}

struct BramblePolicy {
    player: PlayerId,
    bramble_cast: bool,
    creature_cast: bool,
    bramble_aura_cast: bool,
    other_aura_cast: bool,
}

impl BramblePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            bramble_cast: false,
            creature_cast: false,
            bramble_aura_cast: false,
            other_aura_cast: false,
        }
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
                CARD_ID => self.bramble_cast = true,
                OTHER_CREATURE_ID => self.creature_cast = true,
                _ if definition == AURA_ID && targets.first().is_some_and(|target| matches!(target, Target::Permanent(id) if view.own_battlefield.iter().any(|card| card.id == *id && card.definition == Some(CARD_ID)))) => self.bramble_aura_cast = true,
                _ if definition == AURA_ID => self.other_aura_cast = true,
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
                (Color::MANA_ALL
                    .into_iter()
                    .map(|color| u16::from(view.mana_pool.amount(color)))
                    .sum::<u16>()
                    < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(Color::Green)
            });
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for BramblePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.bramble-elemental-attachment-tokens.v1"
        } else {
            "rav.bramble-elemental-opponent.v1"
        }
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("optional trigger choice is visible");
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            pay: choice.ability == TRIGGER_ID,
            target: None,
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
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if self.player == PlayerId(0) {
            if !self.creature_cast
                && !view
                    .hand
                    .iter()
                    .all(|card| card.definition != Some(OTHER_CREATURE_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    OTHER_CREATURE_ID,
                    ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
            if self.creature_cast
                && !self.bramble_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    ManaCost::with_colors(3, [Color::Green, Color::Green]),
                    vec![],
                ) {
                    return action;
                }
            }
            if self.bramble_cast
                && !self.bramble_aura_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(AURA_ID))
                && let Some(target) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    AURA_ID,
                    ManaCost::with_colors(2, [Color::Green]),
                    vec![Target::Permanent(target.id)],
                ) {
                    return action;
                }
            }
            if self.bramble_aura_cast
                && !self.other_aura_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(AURA_ID))
                && let Some(target) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(OTHER_CREATURE_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    AURA_ID,
                    ManaCost::with_colors(2, [Color::Green]),
                    vec![Target::Permanent(target.id)],
                ) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Bramble duel did not end in a clean seat-zero win".to_owned());
    }
    let bramble = capture.bramble.ok_or("Bramble Elemental was not cast")?;
    let other_creature = capture
        .other_creature
        .ok_or("Watchwolf was not cast as a distinct Aura recipient")?;
    if capture.bramble_zone != Some(Zone::Battlefield) {
        return Err("Bramble Elemental did not remain on the battlefield".to_owned());
    }
    if capture.aura_objects.len() != 2 || capture.aura_targets.len() != 2 {
        return Err(format!(
            "expected two exact Moldervine Cloak attachments, got {:?}/{:?}",
            capture.aura_objects, capture.aura_targets
        ));
    }
    let bramble_aura = capture
        .aura_targets
        .iter()
        .find(|(_, target)| *target == bramble)
        .map(|(aura, _)| *aura)
        .ok_or("no Aura became attached to Bramble")?;
    let unrelated_aura = capture
        .aura_targets
        .iter()
        .find(|(_, target)| *target == other_creature)
        .map(|(aura, _)| *aura)
        .ok_or("second Aura did not attach to Watchwolf")?;
    if bramble_aura == unrelated_aura {
        return Err(
            "one Aura identity was incorrectly reused for two attachment events".to_owned(),
        );
    }
    if capture.trigger_incarnations.len() != 1 {
        return Err(format!(
            "expected exactly one Bramble trigger for its own Aura attachment, got {}",
            capture.trigger_incarnations.len()
        ));
    }
    if capture.token_objects.len() != 2 || capture.token_specs_valid != [true, true] {
        return Err(format!(
            "Bramble must create exactly two live green 1/1 Saprolings, got {:?}",
            capture.token_specs_valid
        ));
    }
    if !capture.token_attack_seen {
        return Err("both Saprolings did not participate in later combat".to_owned());
    }

    let bramble_paid = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == bramble && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Green).count() >= 2)).ok_or("exact {3}{G}{G} Bramble spell payment missing")?;
    let bramble_cast = event_index_after(&capture.events, bramble_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == bramble)).ok_or("Bramble cast receipt missing")?;
    let bramble_resolved = event_index_after(
        &capture.events,
        bramble_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == bramble),
    )
    .ok_or("Bramble resolution receipt missing")?;
    let bramble_entered = event_index_after(&capture.events, bramble_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == bramble)).ok_or("Bramble battlefield entry missing")?;
    let aura_paid = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == bramble_aura && colors.len() == 3 && colors.contains(&Color::Green))).ok_or("Moldervine Cloak exact {2}{G} payment missing")?;
    let aura_cast = event_index_after(&capture.events, aura_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == bramble_aura)).ok_or("Bramble's Moldervine Cloak cast receipt missing")?;
    let aura_resolved = event_index_after(
        &capture.events,
        aura_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == bramble_aura),
    )
    .ok_or("Bramble's Moldervine Cloak resolution missing")?;
    let aura_entered = event_index_after(&capture.events, aura_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == bramble_aura)).ok_or("Bramble's Moldervine Cloak entry missing")?;
    let attached = event_index_after(&capture.events, aura_entered, |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == bramble_aura && *target == bramble)).ok_or("exact AuraAttached receipt for Bramble missing")?;
    let trigger = event_index_after(&capture.events, attached, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == bramble)).ok_or("Aura attachment to Bramble did not stack its trigger")?;
    let trigger_incarnation = match capture.events.get(trigger) {
        Some(GameEvent::TriggeredAbilityStacked {
            source_incarnation, ..
        }) => *source_incarnation,
        _ => return Err("Bramble trigger incarnation receipt disappeared".to_owned()),
    };
    if capture.trigger_incarnations != [trigger_incarnation] {
        return Err("Bramble trigger source incarnation changed".to_owned());
    }
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
        return Err("Bramble trigger must emit exactly two TokenCreated receipts".to_owned());
    }
    let ability = event_index_after(&capture.events, trigger, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: TRIGGER_ID } if *source == bramble && *source_incarnation == trigger_incarnation)).ok_or("optional Bramble trigger did not resolve from its exact incarnation")?;
    if !(bramble_paid < bramble_cast
        && bramble_cast < bramble_resolved
        && bramble_resolved < bramble_entered
        && bramble_entered < aura_paid
        && aura_paid < aura_cast
        && aura_cast < aura_resolved
        && aura_resolved < aura_entered
        && aura_entered < attached
        && attached < trigger
        && trigger < token_events[0].0
        && token_events[0].0 < token_events[1].0
        && token_events[1].0 < ability)
    {
        return Err(
            "Bramble cast, Aura attachment, optional trigger, token, and resolution order differs"
                .to_owned(),
        );
    }
    let other_attached = event_index(&capture.events, |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == unrelated_aura && *target == other_creature)).ok_or("Watchwolf Aura attachment receipt missing")?;
    let second_trigger_after_other = event_index_after(
        &capture.events,
        other_attached,
        |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == bramble),
    );
    if second_trigger_after_other.is_some() {
        return Err("an Aura attached to Watchwolf incorrectly triggered Bramble".to_owned());
    }
    let attack = event_index_after(&capture.events, ability, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if capture.token_objects.iter().all(|token| attackers.contains(token)))).ok_or("two live Saprolings did not attack after trigger resolution")?;
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt missing")?;
    if !(ability < attack && attack < end) {
        return Err("Saproling combat did not precede terminal victory".to_owned());
    }
    let mut selected = vec![
        bramble_paid,
        bramble_cast,
        bramble_resolved,
        bramble_entered,
        aura_paid,
        aura_cast,
        aura_resolved,
        aura_entered,
        attached,
        trigger,
        token_events[0].0,
        token_events[1].0,
        ability,
        other_attached,
        attack,
        end,
    ];
    selected.sort_unstable();
    selected.dedup();
    let events = project_selected_events(&capture.events, &selected);
    Ok(json!({
        "id": "rav-bramble-elemental-exact-aura-attachment-two-saprolings-full-game",
        "seed": seed,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "bramble_object": bramble.0,
        "bramble_base_body": "4/4",
        "bramble_aura_object": bramble_aura.0,
        "bramble_aura_target": bramble.0,
        "other_aura_object": unrelated_aura.0,
        "other_aura_target": other_creature.0,
        "exact_source_attachment_only": true,
        "trigger_incarnation": trigger_incarnation,
        "saproling_objects": capture.token_objects.iter().map(|token| token.0).collect::<Vec<_>>(),
        "saprolings_are_exact_green_1_1_creatures": capture.token_specs_valid,
        "both_saprolings_attacked": capture.token_attack_seen,
        "events_digest": event_digest(&capture.events),
        "events": events,
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Bramble Elemental",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "optional-two-saprolings-when-aura-attaches-to-this-permanent",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::AuraBecomesAttachedToSource,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        },
    }
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_bramble_elemental::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Bramble Elemental is missing from the RAV catalog")?;
    if definition != expected_definition() || definition != registered {
        return Err(
            "Bramble Elemental identity or characteristics differ from the task contract"
                .to_owned(),
        );
    }
    let trigger = candidate_bramble_elemental::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Bramble Elemental trigger is missing from the RAV registry")?;
    if trigger != expected_trigger() || trigger != registered {
        return Err("Bramble Elemental must have its optional exact-source Aura-attachment trigger creating two Saprolings".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Bramble Elemental source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Bramble Elemental source is not declarative Rust".to_owned())?;
    let mut functions = BTreeSet::new();
    let mut has_engine_import = false;
    let mut has_std_import = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Bramble Elemental source has unsupported imports".to_owned());
                }
                for path in paths {
                    if path
                        .first()
                        .is_some_and(|part| part == "cardbench_magic_engine")
                    {
                        has_engine_import = true;
                    }
                    if path == ["std", "collections", "BTreeSet"] {
                        has_std_import = true;
                    }
                }
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
                let expected = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => {
                        return Err(
                            "Bramble Elemental source has an unsupported function".to_owned()
                        );
                    }
                };
                let direct = matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, expected));
                if !matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected))
                    || !direct
                    || !functions.insert(item.sig.ident.to_string())
                {
                    return Err(
                        "Bramble Elemental functions must return direct typed literals".to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Bramble Elemental source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if !has_engine_import
        || !has_std_import
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Bramble Elemental source module structure differs from its candidate contract"
                .to_owned(),
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
        return Err("RAV catalog denominator differs from the task contract".to_owned());
    }
    Ok(catalog)
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 154;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
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
fn reference_seed() -> Result<u64, String> {
    let seed = match std::env::var("CARDBENCH_BRAMBLE_ELEMENTAL_SEED") {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BRAMBLE_ELEMENTAL_SEED must be an integer".to_owned())?,
        Err(std::env::VarError::NotPresent) => REFERENCE_SEED,
        Err(error) => return Err(format!("could not read reference seed: {error}")),
    };
    if seed > MAX_REFERENCE_SEED {
        return Err(format!(
            "reference seed {seed} exceeds the bounded maximum {MAX_REFERENCE_SEED}"
        ));
    }
    Ok(seed)
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
