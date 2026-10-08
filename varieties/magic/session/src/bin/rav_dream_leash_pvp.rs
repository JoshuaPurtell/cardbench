//! Full-game RAV Dream Leash authority for its cast-time tapped-target rule.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, ManaAbilityActivation, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
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
const CARD_ID: &str = "RAV-DREAM-LEASH";
const CARD_NAME: &str = "Dream Leash";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const TIDEWATER_ID: &str = "RAV-TIDEWATER-MINION";
const TIDEWATER_ABILITY: &str = "tap-untap-target-permanent";
const SNAPPING_DRAKE_ID: &str = "RAV-SNAPPING-DRAKE";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dream_leash.rs";
const MAX_SEED: u64 = 1024;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dream_leash.rs"]
mod candidate_dream_leash;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dream Leash PvP result: {error}");
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
            eprintln!("Dream Leash PvP authority failed: {error}");
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
            .any(|card| card.collector_number == 45 && card.name == CARD_NAME)
    {
        return Err("RAV #45 identity or catalog denominator changed".to_owned());
    }

    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Dream Leash full-game replay differed".to_owned());
    }

    let tapped_rejection = untapped_target_rejected(Mutation::None)?;
    let missing_tap_mutation_detected =
        tapped_rejection && !untapped_target_rejected(Mutation::RemoveTappedTargetRestriction)?;
    let mutations = json!({
        "untapped_target_rejected_before_payment_or_stack_mutation": tapped_rejection,
        "missing_tapped_target_restriction_detected_by_untapped_control": missing_tap_mutation_detected,
        "source_relative_control_effect_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveControlChange)?,
        "incorrect_mana_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCost)?,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "untapped_target_rejected_before_payment_or_stack_mutation",
        "missing_tapped_target_restriction_detected_by_untapped_control",
        "source_relative_control_effect_removal_detected_in_game",
        "incorrect_mana_cost_detected_in_game",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Dream Leash semantic/source mutation escaped: {key}"
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
        "card": {"collector_number": 45, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "303.4: Aura spells and enchant restrictions",
                "601.2: casting spells and choosing targets",
                "608.2b: target legality at resolution",
                "613.1b: control-changing effects",
                "117: priority and response windows",
                "508.1: attackers become tapped",
                "510: combat damage and terminal duel result"
            ],
            "source_contract": "Original RAV #45 printing: blue {3}{U}{U} Enchantment — Aura; enchant permanent; may be cast only on a tapped permanent; control enchanted permanent.",
            "oracle_reconciliation": "Current Oracle rephrases the restriction as not choosing an untapped permanent as the spell's target as it is cast. The 2005-10-01 ruling clarifies that the restriction is not checked again when the target untaps while the Aura spell is on the stack or after attachment.",
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
    RemoveTappedTargetRestriction,
    RemoveControlChange,
    WrongCost,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    aura: Option<ObjectId>,
    target: Option<ObjectId>,
    target_definition: Option<String>,
    target_tapped_at_cast: Option<bool>,
    target_controller_at_cast: Option<PlayerId>,
    tidewater: Option<ObjectId>,
    tidewater_target: Option<ObjectId>,
    payment_colors: Vec<Color>,
    target_untapped_by_tidewater: bool,
    attached: bool,
    controller_changed: bool,
    final_target_controller: Option<PlayerId>,
    final_target_tapped: Option<bool>,
    final_aura_zone: Option<Zone>,
    rejected_moves: u32,
    p0_life: i64,
    p1_life: i64,
}

fn find_reference() -> Result<(u64, CapturedMatch), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_DREAM_LEASH_SEED") {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_DREAM_LEASH_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no complete Dream Leash tapped-target trace found for seeds 0..={MAX_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    })
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SNAPPING_DRAKE_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 48,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TIDEWATER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-BIRDS-OF-PARADISE".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 12,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Dream Leash pilot", &pilot_deck),
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
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Dream Leash definition is missing from the RAV registry")?;
    *candidate = candidate_dream_leash::definition();
    apply_mutation(candidate, mutation);
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
        Box::new(DreamLeashPolicy::new(PlayerId(0))),
        Box::new(DreamLeashPolicy::new(PlayerId(1))),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        aura: None,
        target: None,
        target_definition: None,
        target_tapped_at_cast: None,
        target_controller_at_cast: None,
        tidewater: None,
        tidewater_target: None,
        payment_colors: vec![],
        target_untapped_by_tidewater: false,
        attached: false,
        controller_changed: false,
        final_target_controller: None,
        final_target_tapped: None,
        final_aura_zone: None,
        rejected_moves: 0,
        p0_life: 0,
        p1_life: 0,
    };
    while !game.is_game_over() {
        if game.turn > 64 || captured.accepted_moves >= 6000 {
            return Err(format!(
                "Dream Leash duel exceeded its bounded horizon at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Dream Leash policy seat missing")?;
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
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Dream Leash move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Dream Leash invariant failed: {error}"))?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.aura = Some(*card);
                    captured.payment_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.aura = Some(*card);
                    if let Some(stack) = game.stack.iter().find(|item| item.card == *card)
                        && let [Target::Permanent(target)] = stack.targets.as_slice()
                    {
                        captured.target = Some(*target);
                        captured.target_definition = game
                            .card_definition(*target)
                            .ok()
                            .map(|item| item.id.to_owned());
                        captured.target_tapped_at_cast =
                            game.object(*target).ok().map(|object| object.tapped);
                        captured.target_controller_at_cast = game.controller_of(*target).ok();
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == TIDEWATER_ID) =>
                {
                    captured.tidewater = Some(*card);
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(1),
                    source,
                    definition,
                    ability,
                    ..
                } if *definition == TIDEWATER_ID && *ability == TIDEWATER_ABILITY => {
                    captured.tidewater = Some(*source);
                    if let Some(stack) = game.stack.iter().find(|item| {
                        item.card == *source && item.ability_id == Some(TIDEWATER_ABILITY)
                    }) && let [Target::Permanent(target)] = stack.targets.as_slice()
                    {
                        captured.tidewater_target = Some(*target);
                    }
                }
                GameEvent::PermanentUntapped { source, card }
                    if Some(*source) == captured.tidewater && Some(*card) == captured.target =>
                {
                    captured.target_untapped_by_tidewater = true;
                }
                GameEvent::AuraAttached { aura, target }
                    if Some(*aura) == captured.aura && Some(*target) == captured.target =>
                {
                    captured.attached = true;
                }
                GameEvent::AttachmentEstablishedWithoutContinuousEffect {
                    attachment,
                    target,
                    kind: cardbench_magic_engine::AttachmentKind::Aura,
                    ..
                } if Some(*attachment) == captured.aura && Some(*target) == captured.target => {
                    captured.attached = true;
                }
                GameEvent::ControllerChanged {
                    source,
                    target,
                    from: PlayerId(1),
                    to: PlayerId(0),
                } if Some(*source) == captured.aura && Some(*target) == captured.target => {
                    captured.controller_changed = true;
                }
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.final_target_controller = captured
        .target
        .and_then(|target| game.controller_of(target).ok());
    captured.final_target_tapped = captured
        .target
        .and_then(|target| game.object(target).ok().map(|object| object.tapped));
    captured.final_aura_zone = captured.aura.and_then(|aura| game.zone_of(aura));
    captured.p0_life = game
        .player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .life;
    captured.p1_life = game
        .player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .life;
    Ok(captured)
}

fn apply_mutation(definition: &mut CardDefinition, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::RemoveTappedTargetRestriction => {
            if let [Effect::AttachSourceToTappedTarget { target, changes }] =
                definition.effects.as_slice()
            {
                definition.effects = vec![Effect::AttachSourceToTarget {
                    target: *target,
                    changes: changes.clone(),
                }];
            }
        }
        Mutation::RemoveControlChange => {
            if let [Effect::AttachSourceToTappedTarget { changes, .. }] =
                definition.effects.as_mut_slice()
            {
                changes.clear();
            }
            if let [Effect::AttachSourceToTarget { changes, .. }] =
                definition.effects.as_mut_slice()
            {
                changes.clear();
            }
        }
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Blue, Color::Blue])
        }
    }
}

fn new_game_with_definition(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Dream Leash is missing from the registry")? = definition;
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn untapped_target_rejected(mutation: Mutation) -> Result<bool, String> {
    let mut definition = candidate_dream_leash::definition();
    apply_mutation(&mut definition, mutation);
    let mut game = new_game_with_definition(definition)?;
    let aura = game
        .add_card(PlayerId(0), CARD_ID, Zone::Hand)
        .map_err(|error| error.to_string())?;
    let target = game
        .put_on_battlefield(PlayerId(1), WATCHWOLF_ID)
        .map_err(|error| error.to_string())?;
    let islands = (0..5)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-ISLAND")
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    game.begin_game().map_err(|error| error.to_string())?;
    for island in islands {
        game.activate_mana_ability(PlayerId(0), island, Color::Blue)
            .map_err(|error| error.to_string())?;
    }
    if game
        .object(target)
        .map_err(|error| error.to_string())?
        .tapped
    {
        return Err("untapped-target control setup unexpectedly tapped its target".to_owned());
    }
    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    Ok(result.is_err()
        && game.zone_of(aura) == Some(Zone::Hand)
        && !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellCast { card, .. } if *card == aura)))
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.aura == right.aura
        && left.target == right.target
        && left.target_definition == right.target_definition
        && left.target_tapped_at_cast == right.target_tapped_at_cast
        && left.tidewater == right.tidewater
        && left.tidewater_target == right.tidewater_target
        && left.payment_colors == right.payment_colors
        && left.target_untapped_by_tidewater == right.target_untapped_by_tidewater
        && left.final_target_controller == right.final_target_controller
        && left.final_target_tapped == right.final_target_tapped
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) {
        return Err("Dream Leash duel did not reach a normal seat-zero win".to_owned());
    }
    let aura = captured.aura.ok_or("Dream Leash was not cast")?;
    let target = captured
        .target
        .ok_or("Dream Leash cast did not retain a target")?;
    let tidewater = captured
        .tidewater
        .ok_or("opponent did not cast Tidewater Minion")?;
    if captured.target_definition.as_deref() != Some(WATCHWOLF_ID)
        || captured.target_tapped_at_cast != Some(true)
        || captured.target_controller_at_cast != Some(PlayerId(1))
        || captured.tidewater_target != Some(target)
        || !captured.target_untapped_by_tidewater
        || !captured.attached
        || !captured.controller_changed
        || captured.final_target_controller != Some(PlayerId(0))
        || captured.final_target_tapped != Some(false)
        || captured.final_aura_zone != Some(Zone::Battlefield)
        || captured.rejected_moves != 0
        || captured.payment_colors.len() != 5
        || captured
            .payment_colors
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            < 2
    {
        return Err("Dream Leash identity, cast-time tapped target, untap response, attachment, controller, or payment evidence is incomplete".to_owned());
    }
    let events = &captured.events;
    let attack = event_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&target)))
        .ok_or("target Watchwolf did not attack and become tapped before the Aura cast")?;
    let paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == aura && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Blue).count() >= 2))
        .ok_or("Dream Leash exact {3}{U}{U} payment receipt missing")?;
    let cast = event_index_after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == aura))
        .ok_or("Dream Leash cast event missing")?;
    let activated = event_index_after(events, cast, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(1), source, definition, ability, .. } if *source == tidewater && *definition == TIDEWATER_ID && *ability == TIDEWATER_ABILITY))
        .ok_or("Tidewater Minion did not activate in response to Dream Leash")?;
    let untapped = event_index_after(events, activated, |event| matches!(event, GameEvent::PermanentUntapped { source, card } if *source == tidewater && *card == target))
        .ok_or("Tidewater Minion did not untap the exact Aura target")?;
    let ability_resolved = event_index_after(events, untapped, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TIDEWATER_ABILITY, .. } if *source == tidewater))
        .ok_or("Tidewater Minion untap ability did not resolve")?;
    let resolved = event_index_after(
        events,
        ability_resolved,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == aura),
    )
    .ok_or("Dream Leash did not resolve after its target became untapped")?;
    let attached = event_index_after(events, resolved, |event| matches!(event, GameEvent::AuraAttached { aura: attached_aura, target: attached_target } if *attached_aura == aura && *attached_target == target)
        || matches!(event, GameEvent::AttachmentEstablishedWithoutContinuousEffect { attachment, target: attached_target, kind: cardbench_magic_engine::AttachmentKind::Aura, .. } if *attachment == aura && *attached_target == target))
        .ok_or("Dream Leash did not attach to the untapped target")?;
    let controller = event_index_after(events, attached, |event| matches!(event, GameEvent::ControllerChanged { source, target: changed, from: PlayerId(1), to: PlayerId(0) } if *source == aura && *changed == target))
        .ok_or("Dream Leash did not take control of its enchanted permanent")?;
    let terminal = event_index_after(events, controller, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("normal terminal seat-zero win receipt missing")?;
    if !(attack < paid
        && paid < cast
        && cast < activated
        && activated < untapped
        && untapped < ability_resolved
        && ability_resolved < resolved
        && resolved < attached
        && attached < controller
        && controller < terminal)
    {
        return Err(
            "Dream Leash cast/untap/attachment/control/terminal event order is invalid".to_owned(),
        );
    }
    let order = [
        attack,
        paid,
        cast,
        activated,
        untapped,
        ability_resolved,
        resolved,
        attached,
        controller,
        terminal,
    ];
    let selected = order.into_iter().collect::<BTreeSet<_>>();
    let events = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id": "rav-dream-leash-tapped-permanent-cast-only-pvp",
        "seed": seed,
        "decks": ["rav-dream-leash-pilot-60", "rav-watchwolf-tidewater-opponent-60"],
        "policies": ["rav.dream-leash-cast-only-tapped-target.v1", "rav.dream-leash-tidewater-untap-response.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "mechanics": {
            "mana_cost": "{3}{U}{U}",
            "payment_colors": captured.payment_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "aura_object": aura.0,
            "target_object": target.0,
            "target_definition": captured.target_definition,
            "target_was_tapped_at_cast": captured.target_tapped_at_cast,
            "target_controller_at_cast": 1,
            "tidewater_source": tidewater.0,
            "tidewater_targeted_exact_aura_target": captured.tidewater_target == Some(target),
            "target_untapped_while_aura_spell_on_stack": captured.target_untapped_by_tidewater,
            "aura_resolved_after_target_untapped": true,
            "aura_attached": captured.attached,
            "controller_changed_from_opponent_to_pilot": captured.controller_changed,
            "target_controller_at_terminal": 0,
            "target_is_untapped_at_terminal": captured.final_target_tapped == Some(false),
            "aura_final_zone": "Battlefield",
            "controller_won": captured.winner == Some(0)
        },
        "ordered_event_sequence": order,
        "events": events,
        "passed": true
    }))
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

struct DreamLeashPolicy {
    player: PlayerId,
    original_watchwolf: Option<ObjectId>,
    tidewater_cast_turn: Option<u32>,
    second_watchwolf_cast: bool,
    leash_cast: bool,
}

impl DreamLeashPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            original_watchwolf: None,
            tidewater_cast_turn: None,
            second_watchwolf_cast: false,
            leash_cast: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let color_counts = [Color::Green, Color::White, Color::Blue].map(|color| {
            view.own_battlefield
                .iter()
                .filter(|card| card.mana_colors.contains(&color))
                .count()
        });
        let wanted = if self.player == PlayerId(0) {
            Color::Blue
        } else if color_counts[0] == 0 {
            Color::Green
        } else if color_counts[1] == 0 {
            Color::White
        } else {
            Color::Blue
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&wanted)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|land| PolicyAction::PlayLand { card: land.id })
    }
}

impl CodePolicy for DreamLeashPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.dream-leash-cast-only-tapped-target.v1"
        } else {
            "rav.dream-leash-tidewater-untap-response.v1"
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
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                let first = self.original_watchwolf.or_else(|| {
                    view.own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(WATCHWOLF_ID))
                        .map(|card| card.id)
                });
                self.original_watchwolf = first;
                first
                    .into_iter()
                    .filter(|id| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == *id && card.can_attack)
                    })
                    .collect()
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
        if self.player == PlayerId(1)
            && view.priority == self.player
            && view
                .stack_spells
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            let source = view.own_battlefield.iter().find(|card| {
                card.definition == Some(TIDEWATER_ID) && !card.tapped && !card.summoning_sick
            });
            let target = self.original_watchwolf.and_then(|id| {
                view.opponent_battlefield
                    .iter()
                    .find(|card| card.id == id && card.tapped)
            });
            if let (Some(source), Some(target)) = (source, target) {
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: TIDEWATER_ABILITY,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            if !self.leash_cast {
                let target = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID) && card.tapped);
                let minion_ready = view.opponent_battlefield.iter().any(|card| {
                    card.definition == Some(TIDEWATER_ID) && !card.tapped && !card.summoning_sick
                });
                if minion_ready
                    && let (Some(aura), Some(target)) = (
                        view.hand
                            .iter()
                            .find(|card| card.definition == Some(CARD_ID)),
                        target,
                    )
                {
                    if let Some(action) = mana_action_for_cost(view, &[Color::Blue, Color::Blue], 5)
                    {
                        return action;
                    }
                    if can_pay_cost(view, &[Color::Blue, Color::Blue], 5) {
                        self.leash_cast = true;
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: aura.id,
                                targets: vec![Target::Permanent(target.id)],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: ManaPaymentSelection {
                                generic: generic_payment(view, &[Color::Blue, Color::Blue], 3),
                                hybrid: vec![],
                            },
                        };
                    }
                }
            } else if let Some(drake) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SNAPPING_DRAKE_ID))
            {
                if let Some(action) = mana_action_for_cost(view, &[Color::Blue], 4) {
                    return action;
                }
                if can_pay_cost(view, &[Color::Blue], 4) {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: drake.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: generic_payment(view, &[Color::Blue], 3),
                            hybrid: vec![],
                        },
                    };
                }
            }
        } else {
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some("RAV-BIRDS-OF-PARADISE"))
                && let Some(bird) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-BIRDS-OF-PARADISE"))
            {
                if let Some(action) = mana_action_for_cost(view, &[Color::Green], 1) {
                    return action;
                }
                if can_pay_cost(view, &[Color::Green], 1) {
                    return cast_action(bird.id, vec![], view, &[Color::Green], 1);
                }
            }
            let watchwolves = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(WATCHWOLF_ID))
                .count();
            if watchwolves == 0
                && let Some(watchwolf) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = mana_action_for_cost(view, &[Color::Green, Color::White], 2) {
                    return action;
                }
                if can_pay_cost(view, &[Color::Green, Color::White], 2) {
                    return cast_action(
                        watchwolf.id,
                        vec![],
                        view,
                        &[Color::Green, Color::White],
                        2,
                    );
                }
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(TIDEWATER_ID))
                && let Some(minion) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TIDEWATER_ID))
            {
                if let Some(action) = mana_action_for_cost(view, &[Color::Blue, Color::Blue], 5) {
                    return action;
                }
                if can_pay_cost(view, &[Color::Blue, Color::Blue], 5) {
                    self.tidewater_cast_turn = Some(view.turn);
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: minion.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: generic_payment(view, &[Color::Blue, Color::Blue], 3),
                            hybrid: vec![],
                        },
                    };
                }
            }
            if !self.second_watchwolf_cast
                && self
                    .tidewater_cast_turn
                    .is_some_and(|turn| view.turn > turn)
                && let Some(watchwolf) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = mana_action_for_cost(view, &[Color::Green, Color::White], 2) {
                    return action;
                }
                if can_pay_cost(view, &[Color::Green, Color::White], 2) {
                    self.second_watchwolf_cast = true;
                    return cast_action(
                        watchwolf.id,
                        vec![],
                        view,
                        &[Color::Green, Color::White],
                        2,
                    );
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn cast_action(
    card: ObjectId,
    targets: Vec<Target>,
    view: &GameView,
    colored: &[Color],
    total: u8,
) -> PolicyAction {
    PolicyAction::CastWithPayment {
        request: CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        chosen_x: None,
        mana_selection: ManaPaymentSelection {
            generic: generic_payment(view, colored, total.saturating_sub(colored.len() as u8)),
            hybrid: vec![],
        },
    }
}

fn can_pay_cost(view: &GameView, colored: &[Color], total: u8) -> bool {
    view.mana_pool.total() >= total
        && colored.iter().all(|color| {
            view.mana_pool.amount(*color)
                >= colored.iter().filter(|required| *required == color).count() as u8
        })
}

fn mana_action_for_cost(view: &GameView, colored: &[Color], total: u8) -> Option<PolicyAction> {
    let mut required = HashMap::<Color, u8>::new();
    for color in colored {
        *required.entry(*color).or_default() += 1;
    }
    for (color, amount) in required {
        if view.mana_pool.amount(color) < amount {
            return mana_source_action(view, color);
        }
    }
    if view.mana_pool.total() < total {
        for color in [
            Color::Blue,
            Color::Green,
            Color::White,
            Color::Black,
            Color::Red,
            Color::Colorless,
        ] {
            if let Some(action) = mana_source_action(view, color) {
                return Some(action);
            }
        }
    }
    None
}

fn mana_source_action(view: &GameView, color: Color) -> Option<PolicyAction> {
    if let Some(land) = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&color)
    }) {
        return Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        });
    }
    view.own_battlefield
        .iter()
        .find(|card| {
            card.definition == Some("RAV-BIRDS-OF-PARADISE") && !card.tapped && !card.summoning_sick
        })
        .map(|bird| PolicyAction::ActivateBoundManaAbility {
            activation: ManaAbilityActivation {
                source: bird.id,
                ability_id: "produce-one-color",
                chosen_color: Some(color),
            },
        })
}

fn generic_payment(view: &GameView, colored: &[Color], count: u8) -> Vec<Color> {
    let mut available: HashMap<Color, u8> = Color::ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in colored {
        if let Some(amount) = available.get_mut(color) {
            *amount = amount.saturating_sub(1);
        }
    }
    let mut selection = Vec::new();
    for color in Color::ALL {
        let amount = available.get(&color).copied().unwrap_or_default();
        for _ in 0..amount {
            if selection.len() == usize::from(count) {
                return selection;
            }
            selection.push(color);
        }
    }
    selection
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_dream_leash::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Dream Leash is absent from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Blue, Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "aura-enchant-permanent-source-controller-control",
                "aura-target-must-be-tapped-when-cast",
            ]
        || definition.effects
            != [Effect::AttachSourceToTappedTarget {
                target: TargetRequirement::Permanent,
                changes: vec![
                    cardbench_magic_engine::ContinuousChange::ChangeControllerToSourceController,
                ],
            }]
    {
        return Err(
            "Dream Leash candidate differs from the exact RAV #45 definition contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Dream Leash candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    if source.len() > 24 * 1024 {
        return Err("Dream Leash candidate source exceeds the boundary".to_owned());
    }
    let file = syn::parse_file(source)
        .map_err(|_| "Dream Leash candidate source is not valid Rust".to_owned())?;
    if file.items.len() != 3
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err(
            "Dream Leash module must contain only two imports and its literal definition"
                .to_owned(),
        );
    }
    let [Item::Use(_), Item::Use(_), definition] = file.items.as_slice() else {
        return Err("Dream Leash module top-level shape changed".to_owned());
    };
    if !matches!(definition, Item::Fn(function)
        if function.sig.ident == "definition"
            && function.sig.inputs.is_empty()
            && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
            && function.block.stmts.len() == 1
            && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.path.is_ident("CardDefinition") && value.rest.is_none()))
    {
        return Err(
            "Dream Leash definition must be one direct crate-visible CardDefinition literal"
                .to_owned(),
        );
    }
    if normalized_source(source) != CANONICAL_CANDIDATE_SOURCE {
        return Err(
            "Dream Leash normalized source differs from its exact literal contract".to_owned(),
        );
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
        "{source}\nfn injected_side_effect() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-dream-leash-side-effect\"), b\"mutation\"); }}\n"
    );
    let compile_valid_by_construction = syn::parse_file(&injected).is_ok();
    let rejected = validate_candidate_source_text(&injected).is_err();
    if !compile_valid_by_construction || !rejected {
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

const CANONICAL_CANDIDATE_SOURCE: &str = r####"//!Candidate-ownedRAV#45DreamLeashidentityandtapped-permanentAuraeffect.usestd::collections::BTreeSet;usecardbench_magic_engine::{CardDefinition,CardType,Color,ContinuousChange,Effect,ManaCost,TargetRequirement,};#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:"RAV-DREAM-LEASH",name:"Dream Leash",set_code:"RAV",mana_cost:ManaCost::with_colors(3,[Color::Blue,Color::Blue]),colors:BTreeSet::from([Color::Blue]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Enchantment]),is_basic_land:false,supported_rules:&["full-rules-fidelity","aura-enchant-permanent-source-controller-control","aura-target-must-be-tapped-when-cast",],power:None,toughness:None,keywords:vec![],effects:vec![Effect::AttachSourceToTappedTarget{target:TargetRequirement::Permanent,changes:vec![ContinuousChange::ChangeControllerToSourceController],}],}}"####;

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
        "qualification": if passed { "pilot_trace_passed; independent review pending" } else { "not_evaluated" }
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
