//! Full-game diagnostic authority for RAV Flame-Kin Zealot #206.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FLAME-KIN-ZEALOT";
const CARD_NAME: &str = "Flame-Kin Zealot";
const HELPER_ID: &str = "RAV-BOROS-RECRUIT";
const TRIGGER_ID: &str = "etb-team-pump-haste";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/flame_kin_zealot.rs";
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 12_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/flame_kin_zealot.rs"]
mod candidate_flame_kin_zealot;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Flame-Kin Zealot result: {error}");
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
            eprintln!("Flame-Kin Zealot PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongPower,
    WrongToughness,
    NoTrigger,
    WrongTriggerCondition,
    RemovePump,
    WrongPumpAmount,
    RemoveHaste,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct CreatureState {
    object: u64,
    definition_id: String,
    power: i32,
    toughness: i32,
    haste: bool,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    zealot: Option<ObjectId>,
    helper: Option<ObjectId>,
    opponent_helper: Option<ObjectId>,
    cast_turn: Option<u32>,
    entry_turn: Option<u32>,
    mana_paid: Vec<Color>,
    trigger_stacked: bool,
    trigger_resolved: bool,
    stack_index: Option<usize>,
    resolved_index: Option<usize>,
    before_team: Vec<CreatureState>,
    after_team: Vec<CreatureState>,
    before_opponents: Vec<CreatureState>,
    after_opponents: Vec<CreatureState>,
    same_turn_attack_index: Option<usize>,
    same_turn_attackers: Vec<ObjectId>,
    same_turn_damage_index: Option<usize>,
}

fn run_authority() -> Result<Value, String> {
    // This guard is deliberately the first operation: no candidate module function runs before
    // its normalized source and AST have been checked.
    validate_candidate_source()?;
    let source_validation = json!({
        "normalized_literal_source_contract_matched": true,
        "ast_literal_shape_matched": true,
        "compile_valid_injected_side_effect_fixture_compiled": true,
        "compile_valid_injected_side_effect_candidate_rejected": injected_side_effect_candidate_rejected(),
        "guard_ran_before_candidate_function_calls": true
    });
    if source_validation["compile_valid_injected_side_effect_candidate_rejected"] != true {
        return Err(
            "Flame-Kin Zealot source guard accepted an injected side-effect candidate".to_owned(),
        );
    }
    validate_candidate_runtime()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.zealot != replay.zealot
        || captured.same_turn_attack_index != replay.same_turn_attack_index
        || captured.before_team != replay.before_team
        || captured.after_team != replay.after_team
    {
        return Err("Flame-Kin Zealot full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "printed_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower)?,
        "base_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness)?,
        "missing_etb_trigger_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTrigger)?,
        "wrong_etb_condition_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTriggerCondition)?,
        "team_pump_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::RemovePump)?,
        "pump_amount_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPumpAmount)?,
        "haste_grant_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::RemoveHaste)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
        "missing_etb_trigger_mutation_detected_in_game",
        "wrong_etb_condition_mutation_detected_in_game",
        "team_pump_removed_mutation_detected_in_game",
        "pump_amount_changed_mutation_detected_in_game",
        "haste_grant_removed_mutation_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!("Flame-Kin Zealot semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":206,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "117 Priority and timing",
                "302.1-302.6 Creature characteristics and summoning sickness",
                "601.2 Casting spells and paying costs",
                "603.2 and 603.6a Triggered abilities and enters-the-battlefield triggers",
                "608.2 Resolving triggered abilities",
                "702.10 Haste"
            ],
            "source_contract":"Original RAV Flame-Kin Zealot #206 is a red-white {1}{R}{R}{W} 2/2 Creature. Its mandatory target-free ETB trigger gives creatures its controller controls +1/+1 and Haste until end of turn. Current Oracle's 'enters' wording is a terminology update from the printed 'comes into play' phrase; resolution-time creature membership is interpreted using the current ruling.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,
        "candidate_source_validation":source_validation,
        "mutation_sensitivity":mutation_sensitivity,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name);
    let id = if card.name == CARD_NAME {
        Some(CARD_ID)
    } else {
        definition.map(|definition| definition.id)
    };
    json!({
        "collector_number":card.collector_number,
        "name":card.name,
        "definition_id":id,
        "engine_status":if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status":if card.name == CARD_NAME { "passed" } else { "not_qualified" },
        "qualification":if card.name == CARD_NAME { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited":false
    })
}

fn validate_candidate_runtime() -> Result<(), String> {
    let candidate = candidate_flame_kin_zealot::definition();
    let registry = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Flame-Kin Zealot is missing from the RAV registry")?;
    if candidate != registry || candidate != expected_definition() {
        return Err("Flame-Kin Zealot identity, {1}{R}{R}{W} cost, or 2/2 body differs from its source contract".to_owned());
    }
    let candidate_trigger = candidate_flame_kin_zealot::triggered_ability();
    let registry_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Flame-Kin Zealot ETB trigger is missing from the RAV registry")?;
    if candidate_trigger != registry_trigger || candidate_trigger != expected_trigger() {
        return Err("Flame-Kin Zealot mandatory team-pump/Haste trigger differs from the RAV source contract".to_owned());
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red, Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-team-pump-haste",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: cardbench_magic_engine::TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                },
                Effect::AddKeywordToControllerCreaturesUntilEndOfTurn {
                    keyword: Keyword::Haste,
                },
            ],
        },
    }
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_FLAME_KIN_ZEALOT_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_FLAME_KIN_ZEALOT_SEED must be an unsigned integer".to_owned()
        })?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no Flame-Kin Zealot full-game trace satisfied the source contract in seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_flame_kin_zealot::definition();
    let mut trigger = candidate_flame_kin_zealot::triggered_ability();
    let mut remove_trigger = false;
    match mutation {
        Mutation::None => {}
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Red, Color::Red, Color::White]);
        }
        Mutation::WrongPower => definition.power = Some(1),
        Mutation::WrongToughness => definition.toughness = Some(3),
        Mutation::NoTrigger => remove_trigger = true,
        Mutation::WrongTriggerCondition => {
            trigger.ability.condition = TriggerCondition::AnotherCreatureDies
        }
        Mutation::RemovePump => trigger.ability.effects.retain(|effect| {
            !matches!(
                effect,
                Effect::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
            )
        }),
        Mutation::WrongPumpAmount => {
            if let Some(Effect::ModifyControllerCreaturesPtUntilEndOfTurn { power, toughness }) =
                trigger.ability.effects.iter_mut().find(|effect| {
                    matches!(
                        effect,
                        Effect::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
                    )
                })
            {
                *power = 2;
                *toughness = 2;
            }
        }
        Mutation::RemoveHaste => trigger.ability.effects.retain(|effect| {
            !matches!(
                effect,
                Effect::AddKeywordToControllerCreaturesUntilEndOfTurn { .. }
            )
        }),
    }
    let mut game = new_rav_game(definition, trigger, remove_trigger)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: HELPER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: HELPER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [&deck0, &deck1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Flame-Kin Zealot duel decks must contain 60 cards each".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ZealotPolicy::new(PlayerId(0), true)),
        Box::new(ZealotPolicy::new(PlayerId(1), false)),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        zealot: None,
        helper: None,
        opponent_helper: None,
        cast_turn: None,
        entry_turn: None,
        mana_paid: vec![],
        trigger_stacked: false,
        trigger_resolved: false,
        stack_index: None,
        resolved_index: None,
        before_team: vec![],
        after_team: vec![],
        before_opponents: vec![],
        after_opponents: vec![],
        same_turn_attack_index: None,
        same_turn_attackers: vec![],
        same_turn_damage_index: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Flame-Kin Zealot game exceeded its bound on turn {}",
                game.turn
            ));
        }
        // A triggered ability gets its own priority window. Snapshot the battlefield while
        // the ETB trigger is on the stack, before the next action can resolve it.
        if captured.trigger_stacked && !captured.trigger_resolved && captured.before_team.is_empty()
        {
            (captured.before_team, captured.before_opponents) = creature_states(&game)?;
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Flame-Kin Zealot policy seat missing")?;
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
                    "Flame-Kin Zealot policy action failed on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut captured)?;
        captured.events.extend_from_slice(&game.event_log[before..]);
    }
    captured.winner = game.winner().map(|winner| winner.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    captured.rejected_moves = 0;
    Ok(captured)
}

fn new_rav_game(
    definition: CardDefinition,
    trigger: TriggeredAbilityBinding,
    remove_trigger: bool,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Flame-Kin Zealot definition registry entry is missing")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Flame-Kin Zealot ETB registry binding is missing")?;
    if remove_trigger {
        triggers.remove(trigger_index);
    } else {
        triggers[trigger_index] = trigger;
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
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut CapturedMatch) -> Result<(), String> {
    let base = captured.events.len();
    for (offset, event) in events.iter().enumerate() {
        let index = base + offset;
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                if captured.zealot.is_none() {
                    captured.zealot = Some(*card);
                    captured.cast_turn = Some(game.turn);
                }
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == HELPER_ID)
                && captured.helper.is_none() =>
            {
                captured.helper = Some(*card)
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == HELPER_ID)
                && captured.opponent_helper.is_none() =>
            {
                captured.opponent_helper = Some(*card)
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if captured.zealot.is_none()
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID)
                && captured.mana_paid.is_empty() =>
            {
                captured.mana_paid = colors.clone()
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.zealot => {
                captured.entry_turn = Some(game.turn);
            }
            GameEvent::TriggeredAbilityStacked {
                controller: PlayerId(0),
                source,
                ability: TRIGGER_ID,
                ..
            } if Some(*source) == captured.zealot && captured.stack_index.is_none() => {
                captured.trigger_stacked = true;
                captured.stack_index = Some(index);
            }
            GameEvent::AbilityResolved {
                source,
                ability: TRIGGER_ID,
                ..
            } if Some(*source) == captured.zealot && captured.resolved_index.is_none() => {
                captured.trigger_resolved = true;
                captured.resolved_index = Some(index);
                let (team, opponents) = creature_states(game)?;
                captured.after_team = team;
                captured.after_opponents = opponents;
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured.same_turn_attack_index.is_none()
                && captured
                    .zealot
                    .is_some_and(|source| attackers.contains(&source))
                && captured.entry_turn == Some(game.turn) =>
            {
                captured.same_turn_attack_index = Some(index);
                captured.same_turn_attackers = attackers.clone();
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                ..
            } if captured.same_turn_damage_index.is_none()
                && captured.zealot == Some(*source)
                && captured.entry_turn == Some(game.turn) =>
            {
                captured.same_turn_damage_index = Some(index)
            }
            _ => {}
        }
    }
    Ok(())
}

fn creature_states(game: &Game) -> Result<(Vec<CreatureState>, Vec<CreatureState>), String> {
    fn for_player(game: &Game, player: PlayerId) -> Result<Vec<CreatureState>, String> {
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let mut records = Vec::new();
        for card in view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Creature))
        {
            let characteristics = game
                .characteristics(card.id)
                .map_err(|error| error.to_string())?;
            records.push(CreatureState {
                object: card.id.0,
                definition_id: card.definition.unwrap_or("unknown").to_owned(),
                power: characteristics
                    .power
                    .ok_or("Flame-Kin creature has no power")?,
                toughness: characteristics
                    .toughness
                    .ok_or("Flame-Kin creature has no toughness")?,
                haste: characteristics.keywords.contains(&Keyword::Haste),
            });
        }
        records.sort_by_key(|record| record.object);
        Ok(records)
    }
    Ok((
        for_player(game, PlayerId(0))?,
        for_player(game, PlayerId(1))?,
    ))
}

struct ZealotPolicy {
    player: PlayerId,
    pilot: bool,
    zealot_cast_requested: bool,
}

impl ZealotPolicy {
    fn new(player: PlayerId, pilot: bool) -> Self {
        Self {
            player,
            pilot,
            zealot_cast_requested: false,
        }
    }

    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }

    fn land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let red_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-MOUNTAIN"))
            .count();
        let white_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-PLAINS"))
            .count();
        let preferred = if red_count < 2 {
            "RAV-MOUNTAIN"
        } else if white_count < 1 {
            "RAV-PLAINS"
        } else if red_count <= white_count {
            "RAV-MOUNTAIN"
        } else {
            "RAV-PLAINS"
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast_recruit_or_make_mana(view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(HELPER_ID))?;
        let hybrid_color = if view.mana_pool.amount(Color::Red) > 0 {
            Some(Color::Red)
        } else if view.mana_pool.amount(Color::White) > 0 {
            Some(Color::White)
        } else {
            None
        };
        if let Some(color) = hybrid_color {
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![],
                    hybrid: vec![color],
                },
            });
        }
        Self::mana_source(view, Color::Red).or_else(|| Self::mana_source(view, Color::White))
    }

    fn zealot_payment(view: &GameView) -> Option<ManaPaymentSelection> {
        if view.mana_pool.amount(Color::Red) < 2 || view.mana_pool.amount(Color::White) < 1 {
            return None;
        }
        let mut unused = Vec::new();
        let mut red = view.mana_pool.amount(Color::Red);
        let mut white = view.mana_pool.amount(Color::White);
        red -= 2;
        white -= 1;
        for (color, amount) in Color::MANA_ALL.into_iter().map(|color| {
            let amount = if color == Color::Red {
                red
            } else if color == Color::White {
                white
            } else {
                view.mana_pool.amount(color)
            };
            (color, amount)
        }) {
            for _ in 0..amount {
                unused.push(color);
            }
        }
        if unused.is_empty() {
            return None;
        }
        Some(ManaPaymentSelection {
            generic: vec![unused[0]],
            hybrid: vec![],
        })
    }

    fn pay_zealot_or_make_mana(view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if let Some(mana_selection) = Self::zealot_payment(view) {
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
        for color in [Color::Red, Color::Red, Color::White] {
            let available = match color {
                Color::Red => view.mana_pool.amount(Color::Red) >= 2,
                Color::White => view.mana_pool.amount(Color::White) >= 1,
                _ => false,
            };
            if !available && let Some(action) = Self::mana_source(view, color) {
                return Some(action);
            }
        }
        let total: u32 = Color::MANA_ALL
            .into_iter()
            .map(|color| u32::from(view.mana_pool.amount(color)))
            .sum();
        if total < 4 {
            return Color::MANA_ALL
                .into_iter()
                .find_map(|color| Self::mana_source(view, color));
        }
        None
    }
}

impl CodePolicy for ZealotPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.flame-kin-zealot-pilot.v1"
        } else {
            "rav.flame-kin-zealot-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: if self.pilot {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    vec![]
                },
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = Self::land(view) {
            return action;
        }
        let recruits = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(HELPER_ID))
            .count();
        if self.pilot && !self.zealot_cast_requested && recruits > 0 {
            if let Some(action) = Self::pay_zealot_or_make_mana(view) {
                if matches!(action, PolicyAction::CastWithPayment { .. }) {
                    self.zealot_cast_requested = true;
                }
                return action;
            }
        }
        let desired_recruits = if self.pilot { 2 } else { 1 };
        if recruits < desired_recruits
            && let Some(action) = Self::cast_recruit_or_make_mana(view)
        {
            return action;
        }
        if self.pilot && !self.zealot_cast_requested && recruits > 0 {
            if let Some(action) = Self::pay_zealot_or_make_mana(view) {
                if matches!(action, PolicyAction::CastWithPayment { .. }) {
                    self.zealot_cast_requested = true;
                }
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(format!(
            "Flame-Kin Zealot full game did not reach a clean P0 win: terminal={} rejected={}",
            captured.terminal, captured.rejected_moves
        ));
    }
    let source = captured.zealot.ok_or("Flame-Kin Zealot was not cast")?;
    if captured.entry_turn != captured.cast_turn {
        return Err("Zealot entry was not recorded on its casting turn".to_owned());
    }
    if captured.mana_paid.len() != 4
        || captured
            .mana_paid
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            < 2
        || !captured.mana_paid.contains(&Color::White)
    {
        return Err(format!(
            "Flame-Kin Zealot did not pay exact {{1}}{{R}}{{R}}{{W}} colored requirements: {:?}",
            captured.mana_paid
        ));
    }
    if !captured.trigger_stacked || !captured.trigger_resolved {
        return Err("Flame-Kin Zealot mandatory ETB trigger did not stack and resolve".to_owned());
    }
    let before_zealot = captured
        .before_team
        .iter()
        .find(|state| state.object == source.0)
        .ok_or("Zealot was absent from the controller team when its trigger stacked")?;
    let after_zealot = captured
        .after_team
        .iter()
        .find(|state| state.object == source.0)
        .ok_or("Zealot was absent from the controller team when its trigger resolved")?;
    if (
        before_zealot.power,
        before_zealot.toughness,
        before_zealot.haste,
    ) != (2, 2, false)
        || (
            after_zealot.power,
            after_zealot.toughness,
            after_zealot.haste,
        ) != (3, 3, true)
    {
        return Err(format!(
            "Zealot did not receive exactly +1/+1 and Haste: before={before_zealot:?} after={after_zealot:?}"
        ));
    }
    if captured.before_team.len() < 2
        || captured.before_team.iter().any(|before| {
            let Some(after) = captured
                .after_team
                .iter()
                .find(|after| after.object == before.object)
            else {
                return true;
            };
            after.power != before.power + 1
                || after.toughness != before.toughness + 1
                || !after.haste
        })
    {
        return Err(format!(
            "not every controller creature at resolution received +1/+1 and Haste: before={:?} after={:?}",
            captured.before_team, captured.after_team
        ));
    }
    if captured.before_opponents.is_empty() || captured.before_opponents != captured.after_opponents
    {
        return Err(format!(
            "Flame-Kin Zealot changed an opposing creature or lacked an opposing comparison: before={:?} after={:?}",
            captured.before_opponents, captured.after_opponents
        ));
    }
    let helper = captured
        .helper
        .ok_or("P0 Boros Recruit helper was not cast")?;
    if !captured
        .before_team
        .iter()
        .any(|state| state.object == helper.0)
    {
        return Err(
            "pre-existing allied Boros Recruit was not present when the ETB trigger stacked"
                .to_owned(),
        );
    }
    let attack = captured
        .same_turn_attack_index
        .ok_or("newly entered Zealot did not attack in the same turn")?;
    let damage = captured
        .same_turn_damage_index
        .ok_or("same-turn Zealot attack dealt no combat damage to P1")?;
    let stack = captured
        .stack_index
        .ok_or("trigger stack event index missing")?;
    let resolved = captured
        .resolved_index
        .ok_or("trigger resolution event index missing")?;
    let events = &captured.events;
    let payment = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 4)).ok_or("Flame-Kin Zealot exact spell payment event missing")?;
    let cast = event_index_after(events, payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Flame-Kin Zealot cast event missing")?;
    let spell_resolved = event_index_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Flame-Kin Zealot spell resolution missing")?;
    let entry = event_index_after(events, spell_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Flame-Kin Zealot battlefield entry missing")?;
    let trigger = event_index(events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: object, ability: TRIGGER_ID, .. } if *object == source)).ok_or("Flame-Kin Zealot ETB trigger stack event missing")?;
    let ability = event_index(events, |event| matches!(event, GameEvent::AbilityResolved { source: object, ability: TRIGGER_ID, .. } if *object == source)).ok_or("Flame-Kin Zealot ETB ability resolution event missing")?;
    let attack_event = attack;
    let damage_event = damage;
    let terminal = event_index_after(events, damage_event, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal game-end event missing after Zealot combat")?;
    if !(payment < cast
        && cast < spell_resolved
        && spell_resolved < entry
        && entry < trigger
        && trigger == stack
        && stack < ability
        && ability == resolved
        && resolved < attack_event
        && attack_event < damage_event
        && damage_event < terminal)
    {
        return Err(format!(
            "Flame-Kin Zealot cast, ETB, same-turn attack, and terminal event order is invalid: payment={payment} cast={cast} resolution={spell_resolved} entry={entry} stack={stack} ability={ability} attack={attack_event} damage={damage_event} end={terminal}"
        ));
    }
    let projected = project_events(events);
    Ok(json!({
        "id":"rav-flame-kin-zealot-etb-team-pump-haste-full-game",
        "seed":seed,
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":captured.rejected_moves,
        "zealot":{"object":source.0,"definition_id":CARD_ID,"collector_number":206,"mana_cost":"{1}{R}{R}{W}","mana_paid":captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"base_power":2,"base_toughness":2,"power_after_trigger":after_zealot.power,"toughness_after_trigger":after_zealot.toughness,"haste_after_trigger":after_zealot.haste},
        "trigger":{"id":TRIGGER_ID,"mandatory":true,"target_count":0,"stacked":captured.trigger_stacked,"resolved":captured.trigger_resolved,"stack_sequence":stack,"resolve_sequence":resolved,"same_turn_attack_sequence":attack,"same_turn_damage_sequence":damage},
        "resolution_membership":{"controller_creatures_before":captured.before_team,"controller_creatures_after":captured.after_team,"opponent_creatures_before":captured.before_opponents,"opponent_creatures_after":captured.after_opponents,"preexisting_helper_object":helper.0,"opponent_helper_object":captured.opponent_helper.map(|object| object.0),"all_controller_creatures_received_plus_one_plus_one_and_haste":true,"opposing_creatures_unchanged":true},
        "events":projected,
        "passed":true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Flame-Kin Zealot source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn injected_side_effect_candidate_rejected() -> bool {
    let Ok(source) = std::fs::read_to_string(SOURCE_PATH) else {
        return false;
    };
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        &format!("pub(crate) fn definition() -> CardDefinition {{ {INJECTED_SIDE_EFFECT}"),
        1,
    );
    injected != source && validate_candidate_source_text(&injected).is_err()
}

const INJECTED_SIDE_EFFECT: &str = "if std::process::id() > 0 { println!(\"injected candidate side effect\"); std::process::exit(0); }";

// This compile-valid fixture is intentionally never called; the candidate-source guard must
// reject an equivalent mutation before definition() or triggered_ability() can run.
#[allow(dead_code, unreachable_code)]
fn compile_valid_injected_side_effect_fixture() -> CardDefinition {
    if std::process::id() > 0 {
        println!("injected candidate side effect");
        std::process::exit(0);
    }
    candidate_flame_kin_zealot::definition()
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Flame-Kin Zealot candidate source is not valid Rust".to_owned())?;
    if normalize_candidate_source(source)? != CANONICAL_CANDIDATE_SOURCE {
        return Err("Flame-Kin Zealot candidate differs from the printed feature contract: normalized source mismatch".to_owned());
    }
    if !candidate_ast_literal_shape(&file) {
        return Err("Flame-Kin Zealot candidate differs from the printed feature contract: AST shape mismatch".to_owned());
    }
    Ok(())
}

fn candidate_ast_literal_shape(file: &syn::File) -> bool {
    if file.shebang.is_some()
        || file.items.len() != 4
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
        || !matches!(file.items.first(), Some(Item::Use(_)))
        || !matches!(file.items.get(1), Some(Item::Use(_)))
    {
        return false;
    }
    let (Some(Item::Fn(definition)), Some(Item::Fn(trigger))) =
        (file.items.get(2), file.items.get(3))
    else {
        return false;
    };
    fn_shape(
        definition,
        "definition",
        "CardDefinition",
        "CardDefinition",
        &[
            "id",
            "name",
            "set_code",
            "mana_cost",
            "colors",
            "mana_colors",
            "card_types",
            "is_basic_land",
            "supported_rules",
            "power",
            "toughness",
            "keywords",
            "effects",
        ],
    ) && fn_shape(
        trigger,
        "triggered_ability",
        "TriggeredAbilityBinding",
        "TriggeredAbilityBinding",
        &["card_definition", "ability"],
    ) && nested_trigger_shape(trigger)
}

fn fn_shape(
    function: &syn::ItemFn,
    name: &str,
    return_type: &str,
    struct_type: &str,
    fields: &[&str],
) -> bool {
    use syn::{ReturnType, Type};
    let visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate") && vis.in_token.is_none());
    let output_matches = matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(&**ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == return_type)));
    let attrs_ok =
        function.attrs.iter().all(|attribute| {
            attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
        }) && function
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("must_use"))
            .count()
            == 1;
    let Some(Stmt::Expr(Expr::Struct(expr), None)) = function.block.stmts.first() else {
        return false;
    };
    visible
        && function.sig.ident == name
        && function.sig.inputs.is_empty()
        && function.sig.generics.params.is_empty()
        && function.sig.generics.where_clause.is_none()
        && function.sig.constness.is_none()
        && function.sig.asyncness.is_none()
        && function.sig.unsafety.is_none()
        && function.sig.abi.is_none()
        && function.sig.variadic.is_none()
        && output_matches
        && attrs_ok
        && function.block.stmts.len() == 1
        && struct_literal_has_fields(expr, struct_type, fields)
}

fn nested_trigger_shape(function: &syn::ItemFn) -> bool {
    let Some(Stmt::Expr(Expr::Struct(binding), None)) = function.block.stmts.first() else {
        return false;
    };
    let Some(ability) = binding.fields.iter().find_map(|field| match &field.member {
        syn::Member::Named(member) if member == "ability" => match &field.expr {
            Expr::Struct(expr) => Some(expr),
            _ => None,
        },
        _ => None,
    }) else {
        return false;
    };
    struct_literal_has_fields(
        ability,
        "TriggeredAbility",
        &[
            "id",
            "condition",
            "mana_cost",
            "optional",
            "targets",
            "effects",
        ],
    )
}

fn struct_literal_has_fields(expression: &syn::ExprStruct, name: &str, expected: &[&str]) -> bool {
    use syn::Member;
    expression
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
        && expression.rest.is_none()
        && expression.fields.len() == expected.len()
        && expression.fields.iter().zip(expected).all(
            |(field, expected)| matches!(&field.member, Member::Named(name) if name == expected),
        )
}

fn normalize_candidate_source(source: &str) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_depth = 0_usize;
    let mut line_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    while index < chars.len() {
        let current = chars[index];
        if line_comment {
            if current == '\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_depth > 0 {
            if current == '/' && chars.get(index + 1) == Some(&'*') {
                block_depth += 1;
                index += 2;
            } else if current == '*' && chars.get(index + 1) == Some(&'/') {
                block_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string || in_char {
            normalized.push(current);
            if current == '\\' {
                if let Some(escaped) = chars.get(index + 1) {
                    normalized.push(*escaped);
                    index += 2;
                    continue;
                }
            } else if (in_string && current == '"') || (in_char && current == '\'') {
                in_string = false;
                in_char = false;
            }
            index += 1;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            line_comment = true;
            index += 2;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            block_depth = 1;
            index += 2;
            continue;
        }
        if current == '"' {
            in_string = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if current == '\'' && has_char_quote(&chars, index + 1) {
            in_char = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if !current.is_whitespace() {
            normalized.push(current);
        }
        index += 1;
    }
    if in_string || in_char || block_depth != 0 {
        return Err("Flame-Kin Zealot source has an unterminated literal or comment".to_owned());
    }
    Ok(normalized)
}

fn has_char_quote(chars: &[char], mut index: usize) -> bool {
    while let Some(character) = chars.get(index) {
        if *character == '\n' {
            return false;
        }
        if *character == '\\' {
            index += 2;
            continue;
        }
        if *character == '\'' {
            return true;
        }
        index += 1;
    }
    false
}

const CANONICAL_CANDIDATE_SOURCE: &str = "usestd::collections::BTreeSet;usecardbench_magic_engine::{CardDefinition,CardType,Color,Effect,Keyword,ManaCost,TriggerCondition,TriggeredAbility,TriggeredAbilityBinding,};#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:\"RAV-FLAME-KIN-ZEALOT\",name:\"Flame-Kin Zealot\",set_code:\"RAV\",mana_cost:ManaCost::with_colors(1,[Color::Red,Color::Red,Color::White]),colors:BTreeSet::from([Color::Red,Color::White]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Creature]),is_basic_land:false,supported_rules:&[\"full-rules-fidelity\",\"colored-cost-casting\",\"base-characteristics\",\"etb-team-pump-haste\",],power:Some(2),toughness:Some(2),keywords:vec![],effects:vec![],}}#[must_use]pub(crate)fntriggered_ability()->TriggeredAbilityBinding{TriggeredAbilityBinding{card_definition:\"RAV-FLAME-KIN-ZEALOT\",ability:TriggeredAbility{id:\"etb-team-pump-haste\",condition:TriggerCondition::EntersBattlefield,mana_cost:ManaCost::new(0),optional:false,targets:vec![],effects:vec![Effect::ModifyControllerCreaturesPtUntilEndOfTurn{power:1,toughness:1,},Effect::AddKeywordToControllerCreaturesUntilEndOfTurn{keyword:Keyword::Haste,},],},}}";
