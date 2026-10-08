//! Isolated full-game PvP authority for RAV Seeds of Strength.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Layer,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SEEDS-OF-STRENGTH";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/seeds_of_strength.rs";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const REFERENCE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/seeds_of_strength.rs"]
mod candidate_seeds_of_strength;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Seeds of Strength result: {error}");
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
            eprintln!("Seeds of Strength PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPump,
    TwoInstructions,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureSnapshot {
    object: ObjectId,
    definition: Option<&'static str>,
    power: Option<i32>,
    toughness: Option<i32>,
    zone: Option<Zone>,
    controller: Option<PlayerId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    spell: Option<ObjectId>,
    targets: Vec<ObjectId>,
    before: Vec<CreatureSnapshot>,
    during: Vec<CreatureSnapshot>,
    after_expiry: Vec<CreatureSnapshot>,
    damage_to_player: Vec<(ObjectId, i32)>,
    card_definition: Option<CardDefinition>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let reference = run_match(REFERENCE_SEED, Mutation::None)?;
    let trace = validate_trace(&reference)?;
    let replay = run_match(REFERENCE_SEED, Mutation::None)?;
    if reference != replay {
        return Err("Seeds of Strength full-game replay was not deterministic".to_owned());
    }
    let wrong_pump = mutation_detected(Mutation::WrongPump)?;
    let two_instructions = mutation_detected(Mutation::TwoInstructions)?;
    let wrong_cost = mutation_detected(Mutation::WrongCost)?;
    if !wrong_pump || !two_instructions || !wrong_cost {
        return Err("a Seeds of Strength semantic mutation escaped the gameplay oracle".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 227, "definition_id": CARD_ID, "name": "Seeds of Strength"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "601.2: instant spell casting and targets",
                "608.2: resolving spells and abilities",
                "613.1g: power/toughness-changing effects",
                "514: cleanup step and end-of-turn expiration",
                "510.1: combat damage"
            ],
            "source_contract": "RAV Seeds of Strength is a green-white {G}{W} Instant with exactly three creature-targeted +1/+1 effects until end of turn",
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
            "wrong_power_toughness_amount_mutation_detected_in_game": wrong_pump,
            "third_targeted_modifier_removed_mutation_detected_in_game": two_instructions,
            "printed_cost_changed_mutation_detected_in_game": wrong_cost,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn mutation_detected(mutation: Mutation) -> Result<bool, String> {
    match run_match(REFERENCE_SEED, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let card = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Seeds of Strength definition is missing from the RAV registry")?;
    *card = candidate_seeds_of_strength::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongPump => {
            card.effects = vec![
                Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 0
                };
                3
            ];
        }
        Mutation::TwoInstructions => {
            card.effects.pop();
        }
        Mutation::WrongCost => {
            card.mana_cost = ManaCost::with_colors(1, [Color::Green, Color::White])
        }
    }
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
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
                count: 1,
            },
            DeckEntry {
                card: ELF_ID.to_owned(),
                count: 3,
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
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, deck) in [(PlayerId(0), &p0_deck), (PlayerId(1), &p1_deck)] {
        let cards: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if cards != 60 {
            return Err(format!(
                "player {} deck has {cards} cards, expected 60",
                player.0
            ));
        }
        game.load_deck_into_library(player, deck)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SeedsPolicy::new(PlayerId(0))),
        Box::new(SeedsPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut spell = None;
    let mut targets = Vec::new();
    let mut before = Vec::new();
    let mut during = Vec::new();
    let mut after_expiry = Vec::new();
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!(
                "Seeds of Strength duel exceeded its bound at turn {}",
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
        let cast_targets = match &action {
            PolicyAction::CastWithPayment { request, .. }
                if player == PlayerId(0)
                    && game
                        .card_definition(request.card)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                Some((
                    request.card,
                    request
                        .targets
                        .iter()
                        .filter_map(|target| match target {
                            Target::Permanent(object) => Some(*object),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                ))
            }
            _ => None,
        };
        if let Some((source, chosen)) = cast_targets {
            spell = Some(source);
            targets = chosen;
            before = snapshot_targets(&game, &targets)?;
        }
        let old_event_count = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Seeds of Strength policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::ContinuousEffectCreated {
                    source,
                    target,
                    layer: Layer::PowerToughness,
                } if Some(*source) == spell && targets.contains(target) => {
                    during = snapshot_targets(&game, &targets)?;
                }
                GameEvent::ContinuousEffectExpired {
                    source,
                    target,
                    layer: Layer::PowerToughness,
                } if Some(*source) == spell && targets.contains(target) => {
                    after_expiry = snapshot_targets(&game, &targets)?;
                }
                _ => {}
            }
        }
    }
    let damage_to_player: Vec<_> = targets
        .iter()
        .filter_map(|target| {
            game.event_log.iter().find_map(|event| match event {
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if source == target => Some((*target, *amount)),
                _ => None,
            })
        })
        .collect();
    let card_definition = spell.and_then(|object| game.card_definition(object).ok().cloned());
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        spell,
        targets,
        before,
        during,
        after_expiry,
        damage_to_player,
        card_definition,
    })
}

fn snapshot_targets(game: &Game, targets: &[ObjectId]) -> Result<Vec<CreatureSnapshot>, String> {
    targets
        .iter()
        .map(|object| {
            let characteristics = game
                .characteristics(*object)
                .map_err(|error| error.to_string())?;
            Ok(CreatureSnapshot {
                object: *object,
                definition: game
                    .card_definition(*object)
                    .ok()
                    .map(|definition| definition.id),
                power: characteristics.power,
                toughness: characteristics.toughness,
                zone: game.zone_of(*object),
                controller: game.controller_of(*object).ok(),
            })
        })
        .collect()
}

struct SeedsPolicy {
    player: PlayerId,
    spell_cast: bool,
    watchwolf_cast: bool,
}

impl SeedsPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            spell_cast: false,
            watchwolf_cast: false,
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

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
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

    fn cast_single(
        &mut self,
        view: &GameView,
        card_id: &'static str,
        cost: ManaCost,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(card_id))?;
            if card_id == WATCHWOLF_ID {
                self.watchwolf_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
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

    fn cast_seeds(&mut self, view: &GameView, targets: Vec<ObjectId>) -> Option<PolicyAction> {
        let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))?;
            self.spell_cast = true;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: targets.into_iter().map(Target::Permanent).collect(),
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
            .copied();
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for SeedsPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.seeds-of-strength.three-creature-pump.v1"
        } else {
            "rav.seeds-of-strength.passive-opponent.v1"
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
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
                .map(|card| card.id)
                .collect();
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
            let creatures: Vec<_> = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .collect();
            if creatures.len() < 3 {
                if !self.watchwolf_cast
                    && !view
                        .own_battlefield
                        .iter()
                        .any(|card| card.definition == Some(WATCHWOLF_ID))
                    && view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(WATCHWOLF_ID))
                    && let Some(action) = self.cast_single(
                        view,
                        WATCHWOLF_ID,
                        ManaCost::with_colors(0, [Color::Green, Color::White]),
                    )
                {
                    return action;
                }
                if view.hand.iter().any(|card| card.definition == Some(ELF_ID))
                    && let Some(action) =
                        self.cast_single(view, ELF_ID, ManaCost::with_colors(0, [Color::Green]))
                {
                    return action;
                }
            }
            if !self.spell_cast
                && creatures.len() >= 3
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                let targets = creatures
                    .iter()
                    .take(3)
                    .map(|card| card.id)
                    .collect::<Vec<_>>();
                if let Some(action) = self.cast_seeds(view, targets) {
                    return action;
                }
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
        return Err("Seeds of Strength duel did not end in a clean P0 win".to_owned());
    }
    let spell = capture.spell.ok_or("Seeds of Strength was not cast")?;
    if !capture
        .card_definition
        .as_ref()
        .is_some_and(|definition| definition == &expected_definition())
    {
        return Err("Seeds of Strength identity, gold color, Instant type, {G}{W}, or three +1/+1 effects differs".to_owned());
    }
    if capture.targets.len() != 3
        || capture
            .targets
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != 3
    {
        return Err("Seeds of Strength must choose three distinct creature objects".to_owned());
    }
    if capture.before.len() != 3 || capture.during.len() != 3 || capture.after_expiry.len() != 3 {
        return Err(
            "each of the three exact targets needs before, boosted, and post-expiration snapshots"
                .to_owned(),
        );
    }
    let mut definition_ids: Vec<_> = capture
        .before
        .iter()
        .filter_map(|snapshot| snapshot.definition)
        .collect();
    definition_ids.sort_unstable();
    let mut expected_ids = vec![ELF_ID, ELF_ID, WATCHWOLF_ID];
    expected_ids.sort_unstable();
    if definition_ids != expected_ids {
        return Err(format!(
            "Seeds targets should be one Watchwolf and two Elves, found {definition_ids:?}"
        ));
    }
    for ((before, during), after) in capture
        .before
        .iter()
        .zip(&capture.during)
        .zip(&capture.after_expiry)
    {
        if before.object != during.object
            || before.object != after.object
            || before.power.is_none()
            || before.toughness.is_none()
            || during.power != before.power.map(|value| value + 1)
            || during.toughness != before.toughness.map(|value| value + 1)
            || after.power != before.power
            || after.toughness != before.toughness
            || before.zone != Some(Zone::Battlefield)
            || after.zone != Some(Zone::Battlefield)
            || before.controller != Some(PlayerId(0))
            || after.controller != Some(PlayerId(0))
        {
            return Err(format!(
                "Seeds did not add +1/+1 to target {} through end of turn and restore its base characteristics",
                before.object.0
            ));
        }
    }
    if capture.damage_to_player.len() != 3
        || capture
            .damage_to_player
            .iter()
            .any(|(_, amount)| *amount <= 0)
    {
        return Err(format!(
            "all three affected creatures must deal combat damage after expiration: {:?}",
            capture.damage_to_player
        ));
    }
    for target in &capture.targets {
        if !capture
            .damage_to_player
            .iter()
            .any(|(object, _)| object == target)
        {
            return Err(format!(
                "target {} did not appear in post-expiration combat damage",
                target.0
            ));
        }
    }

    let paid = event_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Seeds exact {G}{W} spell payment receipt missing")?;
    let cast = event_index_after(&capture.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Seeds SpellCast receipt missing")?;
    let created: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::ContinuousEffectCreated {
                source,
                target,
                layer: Layer::PowerToughness,
            } if *source == spell && capture.targets.contains(target) => Some((index, *target)),
            _ => None,
        })
        .collect();
    if created.len() != 3
        || created
            .iter()
            .map(|(_, target)| *target)
            .collect::<HashSet<_>>()
            .len()
            != 3
    {
        return Err(
            "Seeds must create exactly three layer-seven effects on the selected objects"
                .to_owned(),
        );
    }
    let resolved = event_index_after(
        &capture.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Seeds did not resolve")?;
    let graveyard = event_index_after(&capture.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("Seeds did not move to graveyard after resolution")?;
    let expired: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::ContinuousEffectExpired {
                source,
                target,
                layer: Layer::PowerToughness,
            } if *source == spell && capture.targets.contains(target) => Some((index, *target)),
            _ => None,
        })
        .collect();
    if expired.len() != 3
        || expired
            .iter()
            .map(|(_, target)| *target)
            .collect::<HashSet<_>>()
            .len()
            != 3
    {
        return Err("each Seeds power/toughness effect must expire at end of turn".to_owned());
    }
    let attack = event_index_after(&capture.events, expired.iter().map(|(index, _)| *index).max().unwrap_or(graveyard), |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if capture.targets.iter().all(|target| attackers.contains(target)))).ok_or("all three buffed creatures must attack after the boosts expire")?;
    let mut damage_indices = Vec::new();
    for target in &capture.targets {
        let index = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if source == target && *amount > 0)).ok_or_else(|| format!("target {} did not deal damage to Player 1 after expiration", target.0))?;
        damage_indices.push((index, *target));
    }
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(paid < cast
        && cast
            < created
                .iter()
                .map(|(index, _)| *index)
                .min()
                .unwrap_or(cast)
        && created
            .iter()
            .map(|(index, _)| *index)
            .max()
            .unwrap_or(cast)
            < resolved
        && resolved < graveyard
        && graveyard
            < expired
                .iter()
                .map(|(index, _)| *index)
                .min()
                .unwrap_or(graveyard)
        && expired
            .iter()
            .map(|(index, _)| *index)
            .max()
            .unwrap_or(graveyard)
            < attack
        && damage_indices
            .iter()
            .all(|(index, _)| *index > attack && *index < end))
    {
        return Err("Seeds payment, cast, modifiers, resolution, expiration, combat, and terminal receipts are out of order".to_owned());
    }
    let mut indices = vec![paid, cast, resolved, graveyard, attack, end];
    indices.extend(created.iter().map(|(index, _)| *index));
    indices.extend(expired.iter().map(|(index, _)| *index));
    indices.extend(damage_indices.iter().map(|(index, _)| *index));
    indices.sort_unstable();
    Ok(json!({
        "id":"rav-seeds-of-strength-three-target-pump-combat-full-game",
        "seed": REFERENCE_SEED,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "seeds_object": spell.0,
        "target_objects": capture.targets.iter().map(|object| object.0).collect::<Vec<_>>(),
        "target_definitions": capture.before.iter().map(|snapshot| snapshot.definition).collect::<Vec<_>>(),
        "target_power_toughness_before": capture.before.iter().map(|snapshot| vec![snapshot.power, snapshot.toughness]).collect::<Vec<_>>(),
        "target_power_toughness_during": capture.during.iter().map(|snapshot| vec![snapshot.power, snapshot.toughness]).collect::<Vec<_>>(),
        "target_power_toughness_after_expiry": capture.after_expiry.iter().map(|snapshot| vec![snapshot.power, snapshot.toughness]).collect::<Vec<_>>(),
        "target_damage_to_player": capture.damage_to_player.iter().map(|(object, amount)| json!({"object":object.0,"amount":amount})).collect::<Vec<_>>(),
        "all_three_targets_attacked_after_expiration": true,
        "events_digest": event_digest(&capture.events),
        "events": project_selected_events(&capture.events, &indices),
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Seeds of Strength",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "three-targeted-layer-7-modifiers",
            "partial-target-resolution",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
        ],
    }
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_seeds_of_strength::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Seeds of Strength is missing from the RAV catalog")?;
    if definition != expected_definition() || definition != registered {
        return Err("Seeds of Strength literal is outside the task source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Seeds of Strength source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Seeds of Strength source is not declarative Rust".to_owned())?;
    let expected_imports = BTreeSet::from(
        [
            vec!["cardbench_magic_engine", "CardDefinition"],
            vec!["cardbench_magic_engine", "CardType"],
            vec!["cardbench_magic_engine", "Color"],
            vec!["cardbench_magic_engine", "Effect"],
            vec!["cardbench_magic_engine", "ManaCost"],
            vec!["std", "collections", "BTreeSet"],
        ]
        .map(|path| path.into_iter().map(str::to_owned).collect::<Vec<_>>()),
    );
    let mut imports = BTreeSet::new();
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Seeds of Strength source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function)
                if function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                }) && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && function.sig.ident == "definition"
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none()
                    && function.sig.constness.is_none()
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.abi.is_none()
                    && function.sig.variadic.is_none()
                    && function.block.stmts.len() == 1 =>
            {
                let direct = matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, "CardDefinition"));
                if !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, "CardDefinition"))
                    || !direct
                {
                    return Err(
                        "Seeds of Strength definition must return a direct typed literal"
                            .to_owned(),
                    );
                }
                definitions += 1;
            }
            _ => {
                return Err(
                    "Seeds of Strength source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || definitions != 1 || file.items.len() != 3 {
        return Err(
            "Seeds of Strength module structure differs from the candidate source contract"
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let ids: HashSet<_> = card_definitions()
        .iter()
        .map(|definition| definition.id)
        .collect();
    for card in &catalog {
        if let CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } =
            card.semantic_status
        {
            if !ids.contains(definition_id) {
                return Err(format!(
                    "executable RAV identity {definition_id} is missing its definition"
                ));
            }
        }
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({ "collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "credited": false, "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "trace_status": if passed { "passed" } else { "not_qualified" } })
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| indices.contains(&index).then_some(event))
        .collect()
}
fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{events:?}").bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--output-root" {
            return arguments.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}
