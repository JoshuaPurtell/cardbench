//! Full-game RAV Drift of Phantasms authority for Defender, Flying, and Transmute.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DRIFT-OF-PHANTASMS";
const CARD_NAME: &str = "Drift of Phantasms";
const DRAKE_ID: &str = "RAV-SNAPPING-DRAKE";
const ISLAND_ID: &str = "RAV-ISLAND";
const TRANSMUTE_ID: &str = "transmute";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/drift_of_phantasms.rs";
const MAX_SEED: u64 = 4096;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/drift_of_phantasms.rs"]
mod candidate_drift_of_phantasms;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Drift of Phantasms PvP result: {error}");
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
            eprintln!("Drift of Phantasms PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let source_guard = validate_compile_valid_side_effect_candidate()?;
    validate_candidate_definition()?;

    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
        || !printings
            .iter()
            .any(|card| card.collector_number == 46 && card.name == CARD_NAME)
    {
        return Err("RAV #46 identity or catalog denominator changed".to_owned());
    }

    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Drift of Phantasms full-game replay differed".to_owned());
    }

    let mutations = json!({
        "defender_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveDefender),
        "flying_removal_detected_by_blocking_legality": mutation_detected(seed, Mutation::RemoveFlying),
        "transmute_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveTransmute),
        "wrong_transmute_cost_detected_in_game": mutation_detected(seed, Mutation::WrongTransmuteCost),
        "wrong_creature_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCreatureCost),
        "wrong_power_detected_in_game": mutation_detected(seed, Mutation::WrongPower),
        "wrong_toughness_detected_in_game": mutation_detected(seed, Mutation::WrongToughness),
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "defender_removal_detected_in_game",
        "flying_removal_detected_by_blocking_legality",
        "transmute_removal_detected_in_game",
        "wrong_transmute_cost_detected_in_game",
        "wrong_creature_cost_detected_in_game",
        "wrong_power_detected_in_game",
        "wrong_toughness_detected_in_game",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Drift of Phantasms semantic/source mutation escaped: {key}"
            ));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 46, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and sorcery timing",
                "202.3: mana value",
                "302.4: Defender",
                "702.9: Flying",
                "702.53: Transmute",
                "701.19: library search and shuffle",
                "508-510: combat declarations and damage"
            ],
            "source_contract": "Original RAV #46 printing: blue {2}{U} Creature — Spirit, 0/5, Defender, Flying, and Transmute {1}{U}{U}; its reminder searches for the same converted mana cost, reveals and moves the card to hand, shuffles, and can be played only as a sorcery.",
            "oracle_reconciliation": "Current Oracle changes converted mana cost to mana value and says Transmute only as a sorcery. Search, reveal, hand destination, shuffle, timing, and cost retain the printed semantics.",
            "model_limitation": "Spirit is retained as public identity; CardDefinition does not currently store creature subtypes.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "source_guard": source_guard,
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial remain",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveDefender,
    RemoveFlying,
    RemoveTransmute,
    WrongTransmuteCost,
    WrongCreatureCost,
    WrongPower,
    WrongToughness,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    transmute_source: Option<ObjectId>,
    source_incarnation: Option<u64>,
    transmute_cost: Option<ManaCost>,
    selected_card: Option<ObjectId>,
    selected_definition: Option<&'static str>,
    selection_revealed: bool,
    selected_in_hand: bool,
    search_resolved: bool,
    library_shuffled: bool,
    transmute_resolved: bool,
    search_private: bool,
    hidden_from_opponent: bool,
    activation_player: Option<PlayerId>,
    activation_turn: Option<u32>,
    activation_step: Option<Step>,
    sorcery_timing_valid: bool,
    battlefield_drift: Option<ObjectId>,
    drift_ready_can_attack: Option<bool>,
    drift_attack_declared: bool,
    opponent_drake: Option<ObjectId>,
    opponent_drake_attacked: bool,
    drift_blocked_flying_drake: bool,
    damage_to_drift_from_drake: Option<i32>,
    drift_payment_amount: Option<usize>,
    drift_payment_colors: Vec<Color>,
    source_zone: Option<Zone>,
    selected_zone: Option<Zone>,
    battlefield_drift_zone: Option<Zone>,
    battlefield_drift_power: Option<i32>,
    battlefield_drift_toughness: Option<i32>,
    p0_life: i64,
    p1_life: i64,
}

fn find_reference() -> Result<(u64, CapturedMatch), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_DRIFT_OF_PHANTASMS_SEED") {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_DRIFT_OF_PHANTASMS_SEED must be unsigned".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no complete Drift of Phantasms trace found for seeds 0..={MAX_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: DRAKE_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: ISLAND_ID.to_owned(),
                count: 48,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: DRAKE_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: ISLAND_ID.to_owned(),
                count: 59,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Drift pilot", &pilot_deck),
        ("RAV opponent", &opponent_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Drift of Phantasms is missing from the RAV registry")?;
    *definition = candidate_drift_of_phantasms::definition();
    apply_mutation(definition, mutation);
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DriftPolicy::new(PlayerId(0))),
        Box::new(DriftPolicy::new(PlayerId(1))),
    ];
    let mut capture = CapturedMatch {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        transmute_source: None,
        source_incarnation: None,
        transmute_cost: None,
        selected_card: None,
        selected_definition: None,
        selection_revealed: false,
        selected_in_hand: false,
        search_resolved: false,
        library_shuffled: false,
        transmute_resolved: false,
        search_private: false,
        hidden_from_opponent: true,
        activation_player: None,
        activation_turn: None,
        activation_step: None,
        sorcery_timing_valid: false,
        battlefield_drift: None,
        drift_ready_can_attack: None,
        drift_attack_declared: false,
        opponent_drake: None,
        opponent_drake_attacked: false,
        drift_blocked_flying_drake: false,
        damage_to_drift_from_drake: None,
        drift_payment_amount: None,
        drift_payment_colors: vec![],
        source_zone: None,
        selected_zone: None,
        battlefield_drift_zone: None,
        battlefield_drift_power: None,
        battlefield_drift_toughness: None,
        p0_life: 0,
        p1_life: 0,
    };
    while !game.is_game_over() {
        if game.turn > 64 || capture.accepted_moves >= 6000 {
            return Err(format!(
                "Drift of Phantasms duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        for card in &view.own_battlefield {
            if card.definition == Some(CARD_ID) && !card.summoning_sick {
                capture
                    .drift_ready_can_attack
                    .get_or_insert(card.can_attack);
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Drift policy seat missing")?;
        let action = if view.library_search_choice.is_some() {
            if player == PlayerId(0) {
                capture.search_private = true;
                let opponent_view = game
                    .view_for_player(PlayerId(1))
                    .map_err(|error| error.to_string())?;
                capture.hidden_from_opponent &= opponent_view.library_search_choice.is_none();
                policy.propose_library_search_choice(&view)
            } else {
                policy.propose_library_search_choice(&view)
            }
        } else if let Some(action) = policy.propose_pending_decision(&view) {
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
        match &action {
            PolicyAction::Transmute { card } if player == PlayerId(0) => {
                capture.transmute_source.get_or_insert(*card);
                capture.source_incarnation =
                    game.object(*card).ok().map(|object| object.incarnation);
                capture.activation_player = Some(player);
                capture.activation_turn = Some(view.turn);
                capture.activation_step = Some(view.step);
                capture.sorcery_timing_valid = view.active_player == player
                    && view.priority == player
                    && view.step == Step::PrecombatMain
                    && view.stack_depth == 0;
            }
            PolicyAction::ChooseLibrarySearchCard {
                source, selected, ..
            } if Some(*source) == capture.transmute_source => {
                capture.selected_card = *selected;
            }
            _ => {}
        }
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Drift move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.source_zone = capture.transmute_source.and_then(|card| game.zone_of(card));
    capture.selected_zone = capture.selected_card.and_then(|card| game.zone_of(card));
    capture.battlefield_drift_zone = capture
        .battlefield_drift
        .and_then(|card| game.zone_of(card));
    if let Some(card) = capture.battlefield_drift {
        let characteristics = game
            .characteristics(card)
            .map_err(|error| error.to_string())?;
        capture.battlefield_drift_power = characteristics.power;
        capture.battlefield_drift_toughness = characteristics.toughness;
    }
    capture.p0_life = game
        .player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .life;
    capture.p1_life = game
        .player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .life;
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut CapturedMatch) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: TRANSMUTE_ID,
                mana_cost,
            } if Some(*source) == capture.transmute_source => {
                capture.transmute_cost = Some(mana_cost.clone())
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                ability: TRANSMUTE_ID,
                ..
            } if Some(*source) == capture.transmute_source => {
                capture.source_incarnation = Some(*source_incarnation)
            }
            GameEvent::AbilityResolved {
                source,
                ability: TRANSMUTE_ID,
                ..
            } if Some(*source) == capture.transmute_source => capture.transmute_resolved = true,
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition,
            } if Some(*card) == capture.selected_card => {
                capture.selected_definition = Some(definition);
                capture.selection_revealed = true;
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if Some(*card) == capture.selected_card => capture.selected_in_hand = true,
            GameEvent::LibrarySearchResolved {
                player: PlayerId(0),
                source,
                found: Some(found),
                destination: cardbench_magic_engine::LibrarySearchDestination::Hand,
            } if Some(*source) == capture.transmute_source
                && Some(*found) == capture.selected_card =>
            {
                capture.search_resolved = true
            }
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            } if capture.search_resolved => capture.library_shuffled = true,
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found: Some(found),
            } if Some(*discarded) == capture.transmute_source
                && Some(*found) == capture.selected_card => {}
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == capture.transmute_source => {
                capture.source_zone = Some(Zone::Graveyard)
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.battlefield_drift.get_or_insert(*card);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.drift_payment_amount = Some(colors.len());
                capture.drift_payment_colors = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == DRAKE_ID) =>
            {
                capture.opponent_drake.get_or_insert(*card);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if capture
                .battlefield_drift
                .is_some_and(|drift| attackers.contains(&drift)) =>
            {
                capture.drift_attack_declared = true
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(1),
                attackers,
            } if capture
                .opponent_drake
                .is_some_and(|drake| attackers.contains(&drake)) =>
            {
                capture.opponent_drake_attacked = true
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(0),
                assignments,
            } if assignments.iter().any(|(attacker, blocker)| {
                Some(*attacker) == capture.opponent_drake
                    && Some(*blocker) == capture.battlefield_drift
            }) =>
            {
                capture.drift_blocked_flying_drake = true
            }
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if Some(*source) == capture.opponent_drake
                && Some(*permanent) == capture.battlefield_drift =>
            {
                capture.damage_to_drift_from_drake = Some(*amount)
            }
            _ => {}
        }
    }
    Ok(())
}

fn apply_mutation(definition: &mut CardDefinition, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::RemoveDefender => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Defender),
        Mutation::RemoveFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::RemoveTransmute => definition
            .keywords
            .retain(|keyword| !matches!(keyword, Keyword::Transmute(_))),
        Mutation::WrongTransmuteCost => {
            if let Some(Keyword::Transmute(cost)) = definition
                .keywords
                .iter_mut()
                .find(|keyword| matches!(keyword, Keyword::Transmute(_)))
            {
                *cost = ManaCost::with_colors(2, [Color::Blue, Color::Blue]);
            }
        }
        Mutation::WrongCreatureCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Blue])
        }
        Mutation::WrongPower => definition.power = Some(1),
        Mutation::WrongToughness => definition.toughness = Some(4),
    }
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.transmute_source == right.transmute_source
        && left.selected_card == right.selected_card
        && left.transmute_cost == right.transmute_cost
        && left.battlefield_drift == right.battlefield_drift
        && left.opponent_drake == right.opponent_drake
        && left.drift_blocked_flying_drake == right.drift_blocked_flying_drake
        && left.drift_payment_amount == right.drift_payment_amount
        && left.drift_payment_colors == right.drift_payment_colors
        && left.p0_life == right.p0_life
        && left.p1_life == right.p1_life
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let source = capture
        .transmute_source
        .ok_or("Drift Transmute source is missing")?;
    let selected = capture
        .selected_card
        .ok_or("Drift Transmute found no card")?;
    let drift = capture
        .battlefield_drift
        .ok_or("pilot did not cast Drift of Phantasms")?;
    let opponent_drake = capture
        .opponent_drake
        .ok_or("opponent did not cast Snapping Drake")?;
    let expected_transmute = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
    if capture.winner != Some(0)
        || capture.p0_life <= 0
        || capture.p1_life > 0
        || capture.rejected_moves != 0
        || capture.turns == 0
        || capture.turns > 64
        || source == selected
        || source == drift
        || selected == drift
        || capture.transmute_cost.as_ref() != Some(&expected_transmute)
        || capture.source_incarnation.is_none()
        || capture.selected_definition != Some(CARD_ID)
        || !capture.selection_revealed
        || !capture.selected_in_hand
        || !capture.search_resolved
        || !capture.library_shuffled
        || !capture.transmute_resolved
        || !capture.search_private
        || !capture.hidden_from_opponent
        || capture.activation_player != Some(PlayerId(0))
        || capture.activation_step != Some(Step::PrecombatMain)
        || capture.activation_turn.is_none()
        || !capture.sorcery_timing_valid
        || capture.source_zone != Some(Zone::Graveyard)
        || capture.selected_zone != Some(Zone::Hand)
        || capture.drift_ready_can_attack != Some(false)
        || capture.drift_attack_declared
        || !capture.opponent_drake_attacked
        || !capture.drift_blocked_flying_drake
        || capture.damage_to_drift_from_drake != Some(3)
        || capture.drift_payment_amount != Some(3)
        || capture
            .drift_payment_colors
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            < 1
        || capture.battlefield_drift_zone != Some(Zone::Battlefield)
        || capture.battlefield_drift_power != Some(0)
        || capture.battlefield_drift_toughness != Some(5)
    {
        return Err(format!(
            "Drift trace contract mismatch: winner={:?} source={source:?} selected={selected:?} cost={:?} blocked={} drift_attack={} damage={:?} life={:?}",
            capture.winner,
            capture.transmute_cost,
            capture.drift_blocked_flying_drake,
            capture.drift_attack_declared,
            capture.damage_to_drift_from_drake,
            [capture.p0_life, capture.p1_life]
        ));
    }
    let events = &capture.events;
    let drift_paid = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == drift && colors.len() == 3 && colors.contains(&Color::Blue)))
        .ok_or("Drift spell did not pay the printed {2}{U} symbol count")?;
    let drift_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == drift))
        .ok_or("pilot did not cast the exact Drift object")?;
    let drift_resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == drift),
    )
    .ok_or("the exact Drift spell did not resolve")?;
    let transmute_paid = unique_index(events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: TRANSMUTE_ID, mana_cost } if *paid_source == source && *mana_cost == expected_transmute))
        .ok_or("Transmute cost payment receipt missing")?;
    let discarded = unique_index(events, |event| matches!(event, GameEvent::DiscardedAsAbilityCost { player: PlayerId(0), source: paid_source, card } if *paid_source == source && *card == source))
        .ok_or("Transmute exact discard receipt missing")?;
    let source_moved = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source))
        .ok_or("Transmute source did not move to graveyard")?;
    let activated = unique_index(events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: paid_source, definition: CARD_ID, ability: TRANSMUTE_ID, .. } if *paid_source == source))
        .ok_or("Transmute ability activation receipt missing")?;
    let decision = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                decision: _,
                player: PlayerId(0),
                kind: cardbench_magic_engine::DecisionKind::LibrarySearch,
                visibility: cardbench_magic_engine::DecisionVisibility::Private,
                ..
            }
        )
    })
    .ok_or("private Transmute library-search decision missing")?;
    let decision_id = match &events[decision] {
        GameEvent::DecisionOpened { decision, .. } => *decision,
        _ => unreachable!(),
    };
    let completed = unique_index(events, |event| matches!(event, GameEvent::DecisionCompleted { decision: id, player: PlayerId(0), kind: cardbench_magic_engine::DecisionKind::LibrarySearch } if *id == decision_id))
        .ok_or("Transmute library-search decision completion missing")?;
    let revealed = unique_index(events, |event| matches!(event, GameEvent::CardRevealed { player: PlayerId(0), card, definition: CARD_ID } if *card == selected))
        .ok_or("Transmute result was not revealed")?;
    let moved_hand = unique_index(
        events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == selected),
    )
    .ok_or("Transmute result did not move to hand")?;
    let search = unique_index(events, |event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source: search_source, found: Some(found), destination: cardbench_magic_engine::LibrarySearchDestination::Hand } if *search_source == source && *found == selected))
        .ok_or("Transmute exact library-search resolution missing")?;
    let shuffle = events
        .iter()
        .enumerate()
        .skip(search + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::LibraryShuffled {
                    player: PlayerId(0),
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("Transmute did not shuffle the controller's library")?;
    let transmuted = unique_index(events, |event| matches!(event, GameEvent::Transmuted { player: PlayerId(0), discarded, found: Some(found) } if *discarded == source && *found == selected))
        .ok_or("Transmuted receipt did not bind the exact source and result")?;
    let resolved = unique_index(events, |event| matches!(event, GameEvent::AbilityResolved { source: resolved_source, ability: TRANSMUTE_ID, .. } if *resolved_source == source))
        .ok_or("Transmute ability did not resolve")?;
    let opponent_drake_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == opponent_drake))
        .ok_or("opponent did not cast the exact Snapping Drake")?;
    let opponent_drake_resolved = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == opponent_drake),
    )
    .ok_or("opponent's exact Snapping Drake did not resolve")?;
    let defender_declaration = events
        .iter()
        .enumerate()
        .skip(opponent_drake_resolved + 1)
        .find_map(|(index, event)| match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if !attackers.contains(&drift) => Some(index),
            _ => None,
        })
        .ok_or("ready Drift was not omitted from the pilot's attacker declaration")?;
    let attack = unique_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&opponent_drake)))
        .ok_or("opponent's flying Snapping Drake did not attack")?;
    let blocked = unique_index(events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.contains(&(opponent_drake, drift))))
        .ok_or("Drift did not block the exact flying Snapping Drake")?;
    let damage = unique_index(events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source: damage_source, permanent, amount: 3 } if *damage_source == opponent_drake && *permanent == drift))
        .ok_or("the attacking Drake did not deal combat damage to the Drift blocker")?;
    let terminal = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("normal terminal seat-zero win missing")?;
    let ordered = [
        drift_paid,
        drift_cast,
        drift_resolved,
        transmute_paid,
        discarded,
        source_moved,
        activated,
        decision,
        completed,
        revealed,
        moved_hand,
        search,
        shuffle,
        transmuted,
        resolved,
        opponent_drake_cast,
        opponent_drake_resolved,
        defender_declaration,
        attack,
        blocked,
        damage,
        terminal,
    ];
    if ordered.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("Transmute, combat, and terminal event order is invalid".to_owned());
    }
    let selected_indices = ordered.into_iter().collect::<BTreeSet<_>>();
    let projected = project_events(events)
        .into_iter()
        .filter(|event| selected_indices.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id": "rav-drift-of-phantasms-transmute-and-flying-block-pvp",
        "seed": seed,
        "decks": ["rav-drift-of-phantasms-blue-pilot-60", "rav-single-snapping-drake-opponent-60"],
        "policies": ["rav.drift-transmute-and-defender.v1", "rav.single-flying-drake.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "mechanics": {
            "mana_cost": "{2}{U}",
            "mana_payment_symbol_count": capture.drift_payment_amount,
            "mana_payment_colors": capture.drift_payment_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "power": capture.battlefield_drift_power,
            "toughness": capture.battlefield_drift_toughness,
            "drift_object": drift.0,
            "drift_can_attack_after_summoning_sickness": capture.drift_ready_can_attack,
            "drift_was_declared_attacking": capture.drift_attack_declared,
            "ready_drift_omitted_from_attacker_declaration": true,
            "opponent_drake_object": opponent_drake.0,
            "opponent_drake_attacked": capture.opponent_drake_attacked,
            "drift_blocked_the_flying_drake": capture.drift_blocked_flying_drake,
            "drift_damage_from_drake": capture.damage_to_drift_from_drake,
            "transmute_source_object": source.0,
            "transmute_result_object": selected.0,
            "transmute_result_definition": capture.selected_definition,
            "transmute_source_incarnation": capture.source_incarnation,
            "transmute_cost": "{1}{U}{U}",
            "transmute_activation_player": 0,
            "transmute_activation_turn": capture.activation_turn,
            "transmute_activation_step": "PrecombatMain",
            "sorcery_timing_valid": capture.sorcery_timing_valid,
            "source_moved_to_graveyard": capture.source_zone == Some(Zone::Graveyard),
            "private_library_choice": capture.search_private,
            "hidden_from_opponent_during_choice": capture.hidden_from_opponent,
            "result_revealed": capture.selection_revealed,
            "result_in_hand": capture.selected_in_hand,
            "library_search_resolved": capture.search_resolved,
            "library_shuffled_after_search": capture.library_shuffled,
            "transmute_resolved": capture.transmute_resolved,
            "result_final_zone": format!("{:?}", capture.selected_zone),
            "controller_won": capture.winner == Some(0)
        },
        "ordered_event_sequence": ordered,
        "events": projected,
        "passed": true
    }))
}

fn unique_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

struct DriftPolicy {
    player: PlayerId,
    drift_cast_started: bool,
    transmute_started: bool,
    drake_cast_started: bool,
}

impl DriftPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            drift_cast_started: false,
            transmute_started: false,
            drake_cast_started: false,
        }
    }

    fn play_land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some(ISLAND_ID))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        if view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8) {
            return None;
        }
        let mut available: HashMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = available.get_mut(color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = Color::ALL
                .into_iter()
                .find(|color| available.get(color).copied().unwrap_or_default() > 0)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }

    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let mut missing = None;
        for color in &cost.colored {
            let required = cost.colored.iter().filter(|known| *known == color).count() as u8;
            if view.mana_pool.amount(*color) < required {
                missing = Some(*color);
                break;
            }
        }
        if missing.is_none()
            && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
        {
            missing = Some(Color::Blue);
        }
        Self::mana_source(view, missing?)
    }

    fn cast(&mut self, view: &GameView, card: ObjectId, cost: &ManaCost) -> Option<PolicyAction> {
        if let Some(selection) = Self::payment(view, cost) {
            Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            })
        } else {
            Self::make_mana(view, cost)
        }
    }
}

impl CodePolicy for DriftPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.drift-transmute-and-defender.v1"
        } else {
            "rav.single-flying-drake.v1"
        }
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .library_search_choice
            .as_ref()
            .expect("controller has Transmute choice");
        let selected = choice
            .cards
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
            .map(|card| card.id);
        PolicyAction::ChooseLibrarySearchCard {
            decision: choice.decision,
            source: choice.source,
            selected,
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
                .filter(|card| {
                    card.can_attack
                        && (card.definition == Some(CARD_ID) || card.definition == Some(DRAKE_ID))
                })
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.player == PlayerId(0) {
                view.combat_attackers
                    .iter()
                    .find(|attacker| attacker.definition == Some(DRAKE_ID))
                    .and_then(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| {
                                blocker.definition == Some(CARD_ID) && blocker.can_block
                            })
                            .map(|blocker| CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = Self::play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            let battlefield_drift = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !battlefield_drift
                && !self.drift_cast_started
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) =
                    self.cast(view, card.id, &ManaCost::with_colors(2, [Color::Blue]))
                {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.drift_cast_started = true;
                    }
                    return action;
                }
            }
            if battlefield_drift
                && !self.transmute_started
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let transmute = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
                if Self::payment(view, &transmute).is_some() {
                    self.transmute_started = true;
                    return PolicyAction::Transmute { card: card.id };
                }
                if let Some(action) = Self::make_mana(view, &transmute) {
                    return action;
                }
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(DRAKE_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Blue]);
                if let Some(action) = self.cast(view, card.id, &cost) {
                    return action;
                }
            }
        } else if !self.drake_cast_started
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(DRAKE_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(DRAKE_ID))
        {
            let cost = ManaCost::with_colors(3, [Color::Blue]);
            if let Some(action) = self.cast(view, card.id, &cost) {
                if matches!(action, PolicyAction::CastWithPayment { .. }) {
                    self.drake_cast_started = true;
                }
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_drift_of_phantasms::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Drift of Phantasms is absent from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(0)
        || definition.toughness != Some(5)
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "flying",
                "transmute",
            ]
        || definition.keywords
            != [
                Keyword::Defender,
                Keyword::Flying,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
            ]
    {
        return Err(
            "Drift of Phantasms candidate differs from the exact RAV #46 definition contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Drift source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    if source.len() > 24 * 1024 {
        return Err("Drift candidate source exceeds the boundary".to_owned());
    }
    let file = syn::parse_file(source)
        .map_err(|_| "Drift candidate source is not valid Rust".to_owned())?;
    if file.items.len() != 3
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err(
            "Drift module must contain only two imports and its literal definition".to_owned(),
        );
    }
    let [Item::Use(_), Item::Use(_), definition] = file.items.as_slice() else {
        return Err("Drift source top-level shape changed".to_owned());
    };
    if !matches!(definition, Item::Fn(function)
        if function.sig.ident == "definition" && function.sig.inputs.is_empty()
            && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
            && function.block.stmts.len() == 1
            && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.path.is_ident("CardDefinition") && value.rest.is_none()))
    {
        return Err(
            "Drift definition must be one direct crate-visible CardDefinition literal".to_owned(),
        );
    }
    if normalized_source(source) != CANONICAL_CANDIDATE_SOURCE {
        return Err("Drift normalized source differs from its exact literal contract".to_owned());
    }
    Ok(())
}

fn normalized_source(source: &str) -> String {
    let mut normalized = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in source.chars() {
        if in_string {
            normalized.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
        } else if character == '"' {
            in_string = true;
            normalized.push(character);
        } else if !character.is_whitespace() {
            normalized.push(character);
        }
    }
    normalized
}

fn validate_compile_valid_side_effect_candidate() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let injected = format!(
        "{source}\nfn injected_side_effect() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-drift-side-effect\"), b\"mutation\"); }}\n"
    );
    let compile_valid = syn::parse_file(&injected).is_ok();
    let rejected = validate_candidate_source_text(&injected).is_err();
    if !compile_valid || !rejected {
        return Err(
            "compile-valid side-effect candidate escaped the pre-call source gate".to_owned(),
        );
    }
    Ok(json!({
        "compile_valid_side_effect_candidate_by_construction": true,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": true,
        "candidate_factories_called_for_injected_source": false,
        "sealed_candidate_attestation": false
    }))
}

const CANONICAL_CANDIDATE_SOURCE: &str = r#"//!Candidate-ownedRAV#46DriftofPhantasmsdefinition.usestd::collections::BTreeSet;usecardbench_magic_engine::{CardDefinition,CardType,Color,Keyword,ManaCost};#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:"RAV-DRIFT-OF-PHANTASMS",name:"Drift of Phantasms",set_code:"RAV",mana_cost:ManaCost::with_colors(2,[Color::Blue]),colors:BTreeSet::from([Color::Blue]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Creature]),is_basic_land:false,supported_rules:&["full-rules-fidelity","colored-cost-casting","base-characteristics","defender","flying","transmute",],power:Some(0),toughness:Some(5),keywords:vec![Keyword::Defender,Keyword::Flying,Keyword::Transmute(ManaCost::with_colors(1,[Color::Blue,Color::Blue])),],effects:vec![],}}"#;

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only"
        },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent review pending" } else { "not_evaluated"}
    })
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
