//! Full-game diagnostic authority for RAV Stone-Seeder Hierophant.
//!
//! The trace casts the printed 1/1, taps it to untap a target Forest, and then
//! verifies its controller-scoped land-entry trigger untaps the same source.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings, rav_basic_land_type_bindings,
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
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-STONE-SEEDER-HIEROPHANT";
const CARD_NAME: &str = "Stone-Seeder Hierophant";
const TRIGGER_ID: &str = "landfall-untap-source";
const ACTIVATION_ID: &str = "untap-target-land";
const FOREST_ID: &str = "RAV-FOREST";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs";
const MAX_SEED: u64 = 128;
const MAX_TURNS: u32 = 64;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/stone_seeder_hierophant.rs"]
mod candidate_stone_seeder_hierophant;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Stone-Seeder Hierophant report: {error}");
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
            eprintln!("Stone-Seeder Hierophant PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // The syntactic source guard is the only operation allowed to inspect the
    // candidate before its declarative factories are called.
    validate_candidate_source()?;
    let guard = validate_side_effect_mutation()?;
    validate_candidate_contract()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Stone-Seeder Hierophant full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "base_power_mutation_detected": mutation_detected(seed, Mutation::WrongPower)?,
        "wrong_land_scope_mutation_detected": mutation_detected(seed, Mutation::AnyLand)?,
        "wrong_trigger_effect_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerEffect)?,
        "trigger_binding_removal_detected": mutation_detected(seed, Mutation::RemoveTrigger)?,
        "activation_tap_cost_mutation_detected": mutation_detected(seed, Mutation::NoActivationTap)?,
        "activation_target_mutation_detected": mutation_detected(seed, Mutation::WrongActivationTarget)?,
        "activation_effect_mutation_detected": mutation_detected(seed, Mutation::WrongActivationEffect)?,
        "activation_binding_removal_detected": mutation_detected(seed, Mutation::RemoveActivation)?,
        "parse_valid_side_effect_source_rejected_before_candidate_calls": guard["side_effect_mutation_parse_valid"] == true && guard["side_effect_mutation_rejected"] == true && guard["candidate_calls_before_guard"] == 0
    });
    for key in [
        "printed_cost_mutation_detected",
        "base_power_mutation_detected",
        "wrong_land_scope_mutation_detected",
        "wrong_trigger_effect_mutation_detected",
        "trigger_binding_removal_detected",
        "activation_tap_cost_mutation_detected",
        "activation_target_mutation_detected",
        "activation_effect_mutation_detected",
        "activation_binding_removal_detected",
        "parse_valid_side_effect_source_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Stone-Seeder Hierophant mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":184,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true,
            "source_contract":"Original RAV #184: green {2}{G}{G} 1/1 Creature — Human Druid; {T}: untap target land; a land entering under your control untaps Stone-Seeder Hierophant.",
            "oracle_reconciliation":"Current Oracle uses Landfall and enters/this-creature terminology; the controller scope and effects are unchanged. The card has no mana ability.",
            "subtype_model_limit":"Human and Druid are retained in task metadata; CardDefinition currently cannot assert creature subtypes.",
            "checked_sections":["305.1-305.2: land characteristics and playing a land","603.2-603.3: trigger event and stacking","608.2: ability resolution","602.1-602.2: activation and costs","601.2f: mana payment"]
        },
        "source_guard":guard,
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic; independent reviewer approval and candidate trial remain separate",
        "replay_deterministic":true,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongPower,
    AnyLand,
    WrongTriggerEffect,
    RemoveTrigger,
    NoActivationTap,
    WrongActivationTarget,
    WrongActivationEffect,
    RemoveActivation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActivationReceipt {
    source: ObjectId,
    target: ObjectId,
    event_index: usize,
    tapped_before: bool,
    tapped_after: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LandEntryReceipt {
    event_index: usize,
    card: ObjectId,
    controller: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    source_incarnation: Option<u64>,
    source_cost: Option<ManaCost>,
    cast_payment: Vec<Color>,
    source_resolved: bool,
    source_entered: bool,
    source_zone: Option<Zone>,
    base_characteristics: Option<(i32, i32)>,
    land_entries: Vec<LandEntryReceipt>,
    activations: Vec<ActivationReceipt>,
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
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
        || !catalog
            .iter()
            .any(|card| card.collector_number == 184 && card.name == CARD_NAME)
    {
        return Err(
            "RAV catalog identity or denominator differs for Stone-Seeder Hierophant".to_owned(),
        );
    }
    Ok(catalog)
}

fn definition_with_mutation(mutation: Mutation) -> cardbench_magic_engine::CardDefinition {
    let mut definition = candidate_stone_seeder_hierophant::definition();
    match mutation {
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Green, Color::Green])
        }
        Mutation::WrongPower => definition.power = Some(2),
        _ => {}
    }
    definition
}

fn trigger_with_mutation(
    mutation: Mutation,
) -> Option<cardbench_magic_engine::TriggeredAbilityBinding> {
    if mutation == Mutation::RemoveTrigger {
        return None;
    }
    let mut trigger = candidate_stone_seeder_hierophant::triggered_ability();
    match mutation {
        Mutation::AnyLand => {
            trigger.ability.condition =
                cardbench_magic_engine::TriggerCondition::LandEntersBattlefield
        }
        Mutation::WrongTriggerEffect => trigger.ability.effects.clear(),
        _ => {}
    }
    Some(trigger)
}

fn activation_with_mutation(
    mutation: Mutation,
) -> Option<cardbench_magic_engine::ActivatedAbilityBinding> {
    if mutation == Mutation::RemoveActivation {
        return None;
    }
    let mut binding = candidate_stone_seeder_hierophant::activated_ability();
    match mutation {
        Mutation::NoActivationTap => binding.ability.tap_cost = false,
        Mutation::WrongActivationTarget => {
            binding.ability.targets = vec![cardbench_magic_engine::TargetRequirement::Creature]
        }
        Mutation::WrongActivationEffect => binding.ability.effects.clear(),
        _ => {}
    }
    Some(binding)
}

fn validate_candidate_contract() -> Result<(), String> {
    let definition = candidate_stone_seeder_hierophant::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Stone-Seeder Hierophant is missing from the RAV definition registry")?;
    let activation = candidate_stone_seeder_hierophant::activated_ability();
    let registered_activation = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ACTIVATION_ID)
        .ok_or("Stone-Seeder Hierophant activation is missing from the RAV registry")?;
    let trigger = candidate_stone_seeder_hierophant::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Stone-Seeder Hierophant trigger is missing from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green, Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "controlled-land-entry-untaps-source",
                "tap-untap-target-land",
            ]
        || activation != registered_activation
        || activation.ability.id != ACTIVATION_ID
        || activation.ability.mana_cost != ManaCost::new(0)
        || !activation.ability.tap_cost
        || activation.ability.sorcery_speed
        || activation.ability.additional_tap_creatures != 0
        || activation.ability.sacrifice_source
        || activation.ability.sacrifice_creatures != 0
        || activation.ability.sacrifice_lands != 0
        || activation.ability.discard_cards != 0
        || activation.ability.targets != [cardbench_magic_engine::TargetRequirement::Land]
        || activation.ability.effects != [Effect::UntapTargetLand]
        || trigger != registered_trigger
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition
            != cardbench_magic_engine::TriggerCondition::ControlledLandEntersBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::UntapSource]
    {
        return Err(
            "Stone-Seeder Hierophant candidate or registry differs from the printed RAV contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_STONE_SEEDER_HIEROPHANT_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_STONE_SEEDER_HIEROPHANT_SEED must be an unsigned integer")?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Stone-Seeder Hierophant full-game trace found for seeds 0..={MAX_SEED}: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Stone-Seeder Hierophant is absent from the RAV registry")? =
        definition_with_mutation(mutation);
    let mut activated = rav_activated_ability_bindings();
    activated.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(binding) = activation_with_mutation(mutation) {
        activated.push(binding);
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(binding) = trigger_with_mutation(mutation) {
        triggers.push(binding);
    }
    triggers.extend(rav_attachment_triggered_ability_bindings());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated,
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

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (FOREST_ID, 56)]);
    let opponent = deck(&[(FOREST_ID, 60)]);
    for (label, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        let size: usize = list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!(
                "Stone-Seeder Hierophant {label} deck has {size} cards, expected 60"
            ));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..Default::default()
    };
    let mut game = new_game(mutation)?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(HierophantPolicy::new(PlayerId(0), mutation)),
        Box::new(LandPolicy {
            player: PlayerId(1),
        }),
    ];
    let mut accepted = 0_u32;
    let mut attempts = 0_u32;
    let mut activations = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Stone-Seeder Hierophant duel exceeded its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Stone-Seeder policy seat missing")?;
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
        let activation_target = match &action {
            PolicyAction::ActivateAbility { activation }
                if activation.ability_id == ACTIVATION_ID =>
            {
                activation.targets.iter().find_map(|target| {
                    if let Target::Permanent(id) = target {
                        Some((activation.source, *id))
                    } else {
                        None
                    }
                })
            }
            _ => None,
        };
        let source_tapped_before = activation_target
            .and_then(|(source, _)| game.object(source).ok().map(|object| object.tapped));
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if let Some((source, target)) = activation_target {
            if let Some(event_index) = game.event_log[cursor..].iter().position(|event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: event_source, definition: CARD_ID, ability: ACTIVATION_ID, .. } if *event_source == source)).map(|index| cursor + index) {
                activations.push(ActivationReceipt { source, target, event_index, tapped_before: source_tapped_before.unwrap_or(false), tapped_after: game.object(source).is_ok_and(|object| object.tapped) });
            }
        }
    }
    let events = game.event_log.clone();
    let source = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let source_incarnation =
        source.and_then(|card| game.object(card).ok().map(|object| object.incarnation));
    let source_cost = source.and_then(|card| {
        game.card_definition(card)
            .ok()
            .map(|definition| definition.mana_cost.clone())
    });
    let cast_payment = source
        .and_then(|source| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == source => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let source_resolved = source.is_some_and(|source| {
        events
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == source))
    });
    let source_entered = source.is_some_and(|source| events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)));
    let source_zone = source.and_then(|card| game.zone_of(card));
    let base_characteristics = source
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let source_entry_index = source.and_then(|source| events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)));
    let land_entries = events
        .iter()
        .enumerate()
        .filter_map(|(event_index, event)| {
            let GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } = event
            else {
                return None;
            };
            let after = source_entry_index.is_some_and(|index| event_index > index);
            (after
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.card_types.contains(&CardType::Land)))
            .then(|| {
                game.controller_of(*card)
                    .ok()
                    .map(|controller| LandEntryReceipt {
                        event_index,
                        card: *card,
                        controller: controller.0,
                    })
            })
            .flatten()
        })
        .collect();
    Ok(Captured {
        events,
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempts.saturating_sub(accepted),
        source,
        source_incarnation,
        source_cost,
        cast_payment,
        source_resolved,
        source_entered,
        source_zone,
        base_characteristics,
        land_entries,
        activations,
    })
}

struct HierophantPolicy {
    player: PlayerId,
    cast_once: bool,
    mutation: Mutation,
    activation_turn: u32,
    activations_this_turn: u8,
}

impl HierophantPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            cast_once: false,
            mutation,
            activation_turn: 0,
            activations_this_turn: 0,
        }
    }

    fn source<'a>(view: &'a GameView) -> Option<&'a cardbench_magic_engine::CardView> {
        view.own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
    }

    fn forest_mana(view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| card.definition == Some(FOREST_ID) && !card.tapped)
            .map(|forest| PolicyAction::ActivateManaAbility {
                land: forest.id,
                color: Color::Green,
            })
    }

    fn target_land(view: &GameView) -> Option<ObjectId> {
        view.own_battlefield
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land) && card.tapped)
            .map(|card| card.id)
    }

    fn activate(&mut self, source: ObjectId, target: ObjectId) -> PolicyAction {
        self.activations_this_turn = self.activations_this_turn.saturating_add(1);
        PolicyAction::ActivateAbility {
            activation: cardbench_magic_engine::AbilityActivation {
                source,
                ability_id: ACTIVATION_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(target)],
            },
        }
    }
}

impl CodePolicy for HierophantPolicy {
    fn id(&self) -> &'static str {
        "rav.stone-seeder-hierophant-full-game-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
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
        if view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if self.activation_turn != view.turn {
            self.activation_turn = view.turn;
            self.activations_this_turn = 0;
        }
        if let Some(source) = Self::source(view) {
            if source.summoning_sick {
                if view.lands_played == 0
                    && let Some(forest) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(FOREST_ID))
                {
                    return PolicyAction::PlayLand { card: forest.id };
                }
                return PolicyAction::PassPriority;
            }
            if source.tapped
                && self.activations_this_turn == 0
                && view.lands_played == 0
                && let Some(forest) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(FOREST_ID))
            {
                return PolicyAction::PlayLand { card: forest.id };
            }
            if self.activations_this_turn == 0 {
                if let Some(target) = Self::target_land(view) {
                    return self.activate(source.id, target);
                }
                if let Some(action) = Self::forest_mana(view) {
                    return action;
                }
                if view.lands_played == 0
                    && let Some(forest) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(FOREST_ID))
                {
                    return PolicyAction::PlayLand { card: forest.id };
                }
            }
            if self.activations_this_turn == 1 && source.tapped && view.lands_played == 0 {
                if let Some(forest) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(FOREST_ID))
                {
                    return PolicyAction::PlayLand { card: forest.id };
                }
            }
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(forest) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FOREST_ID))
        {
            return PolicyAction::PlayLand { card: forest.id };
        }
        if !self.cast_once
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = definition_with_mutation(self.mutation).mana_cost;
            if let Some(selection) = payment(view, &cost) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = Self::forest_mana(view) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

struct LandPolicy {
    player: PlayerId,
}
impl CodePolicy for LandPolicy {
    fn id(&self) -> &'static str {
        "rav.stone-seeder-hierophant-opponent-land-drop.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority == self.player
            && view.active_player == self.player
            && view.step.is_main()
            && view.stack_depth == 0
            && view.lands_played == 0
            && let Some(forest) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FOREST_ID))
        {
            return PolicyAction::PlayLand { card: forest.id };
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
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

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0)
        || capture.rejected_moves != 0
        || capture.source_zone != Some(Zone::Battlefield)
    {
        return Err(
            "Stone-Seeder Hierophant did not reach a clean Player 0 win with the source alive"
                .to_owned(),
        );
    }
    let source = capture
        .source
        .ok_or("Stone-Seeder Hierophant was not cast")?;
    let incarnation = capture
        .source_incarnation
        .ok_or("Stone-Seeder source incarnation missing")?;
    if !capture.source_resolved
        || !capture.source_entered
        || capture.source_cost != Some(ManaCost::with_colors(2, [Color::Green, Color::Green]))
        || capture.cast_payment.len() != 4
        || capture
            .cast_payment
            .iter()
            .any(|color| *color != Color::Green)
        || capture.base_characteristics != Some((1, 1))
    {
        return Err(
            "Stone-Seeder Hierophant did not cast for {2}{G}{G} and enter as a 1/1".to_owned(),
        );
    }
    let events = &capture.events;
    let entry = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Stone-Seeder Hierophant battlefield-entry receipt missing")?;
    let paid = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 4 && colors.iter().all(|color| *color == Color::Green))).ok_or("exact four-green cast-payment receipt missing")?;
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Stone-Seeder SpellCast receipt missing")?;
    let resolved_spell = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Stone-Seeder SpellResolved receipt missing")?;
    if !(paid < cast && cast < resolved_spell && resolved_spell < entry) {
        return Err("Stone-Seeder cast, payment, resolution, and entry order differs".to_owned());
    }
    let own_lands: Vec<_> = capture
        .land_entries
        .iter()
        .filter(|land| land.controller == 0 && land.event_index > entry)
        .collect();
    let opposing_lands: Vec<_> = capture
        .land_entries
        .iter()
        .filter(|land| land.controller == 1 && land.event_index > entry)
        .collect();
    if own_lands.is_empty() || opposing_lands.is_empty() {
        return Err(
            "full-game trace needs both a post-entry controller and opponent land entry".to_owned(),
        );
    }
    let trigger_receipts: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::TriggeredAbilityStacked {
                controller: PlayerId(0),
                source: event_source,
                source_incarnation,
                ability: TRIGGER_ID,
            } if *event_source == source => Some((index, *source_incarnation)),
            _ => None,
        })
        .collect();
    let trigger_resolutions: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::AbilityResolved {
                source: event_source,
                source_incarnation,
                ability: TRIGGER_ID,
            } if *event_source == source => Some((index, *source_incarnation)),
            _ => None,
        })
        .collect();
    let trigger_untaps: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::PermanentUntapped {
                source: event_source,
                card,
            } if *event_source == source && *card == source => Some(index),
            _ => None,
        })
        .collect();
    if trigger_receipts.len() != own_lands.len()
        || trigger_resolutions.len() != own_lands.len()
        || trigger_untaps.len() != own_lands.len()
    {
        return Err(format!(
            "controller-land entries and source untap triggers differ: own={}, opponent={}, stacked={}, resolved={}, untapped={}",
            own_lands.len(),
            opposing_lands.len(),
            trigger_receipts.len(),
            trigger_resolutions.len(),
            trigger_untaps.len()
        ));
    }
    for (index, land) in own_lands.iter().enumerate() {
        let (stacked, trigger_incarnation) = trigger_receipts[index];
        let (resolved, resolve_incarnation) = trigger_resolutions[index];
        if !(land.event_index < stacked
            && stacked < trigger_untaps[index]
            && trigger_untaps[index] < resolved)
            || trigger_incarnation != incarnation
            || resolve_incarnation != incarnation
        {
            return Err(
                "controller land entry did not untap the same source incarnation through the stack"
                    .to_owned(),
            );
        }
    }
    let mut activation_order = Vec::new();
    for receipt in &capture.activations {
        if receipt.source != source
            || receipt.tapped_before
            || !receipt.tapped_after
            || receipt.target == source
        {
            return Err(
                "tap ability must tap its untapped source and target a different land".to_owned(),
            );
        }
        let resolved = first_after(events, receipt.event_index, |event| matches!(event, GameEvent::AbilityResolved { source: event_source, source_incarnation: event_incarnation, ability: ACTIVATION_ID } if *event_source == source && *event_incarnation == incarnation)).ok_or("target-land activation did not resolve")?;
        let target_untap = first_after(events, receipt.event_index, |event| matches!(event, GameEvent::PermanentUntapped { source: event_source, card } if *event_source == source && *card == receipt.target)).ok_or("target land did not untap from its activation")?;
        if !(receipt.event_index < target_untap && target_untap < resolved) {
            return Err("target land did not untap during ability resolution".to_owned());
        }
        activation_order.push((receipt.event_index, target_untap, resolved, receipt.target));
    }
    if activation_order.is_empty() {
        return Err("Stone-Seeder target-land ability was never activated".to_owned());
    }
    if capture
        .activations
        .iter()
        .any(|receipt| !receipt.tapped_before || !receipt.tapped_after)
    {
        return Err("Stone-Seeder tap cost did not tap its source".to_owned());
    }
    for opponent_entry in &opposing_lands {
        let next_land = capture
            .land_entries
            .iter()
            .filter(|land| land.event_index > opponent_entry.event_index)
            .map(|land| land.event_index)
            .min()
            .unwrap_or(events.len());
        if trigger_receipts.iter().any(|(trigger_index, _)| {
            *trigger_index > opponent_entry.event_index && *trigger_index < next_land
        }) {
            return Err(
                "opponent land entry incorrectly triggered Stone-Seeder Hierophant".to_owned(),
            );
        }
    }
    let final_trigger_resolution = trigger_resolutions
        .last()
        .map(|receipt| receipt.0)
        .ok_or("controller landfall resolution missing")?;
    let attack = first_after(events, final_trigger_resolution, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).ok_or("Stone-Seeder Hierophant never attacked after its source-untap trigger")?;
    let damage = first_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount } if *damage_source == source && *amount > 0)).ok_or("Stone-Seeder Hierophant dealt no combat damage")?;
    let ended = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal Player 0 win receipt missing")?;
    if !(resolved_spell < attack && attack < damage && damage < ended) {
        return Err("Stone-Seeder full-game combat and terminal event order differs".to_owned());
    }
    let projected = project_events(events);
    let sequence: Vec<_> = projected.iter().map(|event| event.sequence).collect();
    let witness_events: Vec<_> = projected
        .iter()
        .map(|event| serde_json::to_value(event).expect("projected event serializes"))
        .collect();
    Ok(json!({
        "id":"rav-stone-seeder-hierophant-landfall-target-untap-full-game",
        "seed":seed,
        "decks":["rav-stone-seeder-hierophant-pilot-60","rav-basic-forest-opponent-60"],
        "policies":["rav.stone-seeder-hierophant-full-game-pilot.v1","rav.stone-seeder-hierophant-opponent-land-drop.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":capture.turns,
        "accepted_moves":capture.accepted_moves,"rejected_moves":capture.rejected_moves,
        "mechanics":{
            "definition_id":CARD_ID,"source_object":source.0,"source_incarnation":incarnation,
            "mana_cost":"{2}{G}{G}","paid_mana_colors":capture.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "base_power":1,"base_toughness":1,"has_mana_ability":false,
            "controller_land_entries_after_entry":own_lands.iter().map(|land|land.card.0).collect::<Vec<_>>(),
            "opponent_land_entries_after_entry":opposing_lands.iter().map(|land|land.card.0).collect::<Vec<_>>(),
            "trigger_count":trigger_receipts.len(),"source_untap_receipt_count":trigger_untaps.len(),
            "opponent_land_triggered_source":false,
            "activation_count":capture.activations.len(),
            "activation_targets":activation_order.iter().map(|item|item.3.0).collect::<Vec<_>>(),
            "activation_source_tapped_for_each_cost":capture.activations.iter().all(|receipt|!receipt.tapped_before&&receipt.tapped_after),
            "target_land_untap_receipt_count":activation_order.len(),
            "terminal_pilot_win":true
        },
        "ordered_event_sequence":sequence,"events":witness_events,"passed":true
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Stone-Seeder candidate source: {error}"))?;
    validate_source_shape(&source)
}

fn validate_side_effect_mutation() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let needle = "pub(crate) fn definition() -> CardDefinition {\n    CardDefinition {";
    let injected = source.replace(needle, "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/stone-seeder-guard\", \"bad\");\n    CardDefinition {");
    if injected == source {
        return Err("could not construct Stone-Seeder side-effect source mutation".to_owned());
    }
    let parse_valid = syn::parse_file(&injected).is_ok();
    let rejected = validate_source_shape(&injected).is_err();
    if !parse_valid || !rejected {
        return Err("parse-valid Stone-Seeder side-effect source was not rejected".to_owned());
    }
    Ok(
        json!({"side_effect_mutation_parse_valid":true,"side_effect_mutation_rejected":true,"side_effect_mutation_rejected_before_candidate_calls":true,"candidate_calls_before_guard":0,"compile_validity":"not_claimed; no Cargo/compiler run was authorized"}),
    )
}

fn validate_source_shape(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Stone-Seeder source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Stone-Seeder module contains an unsupported attribute".to_owned());
    }
    let mut functions = BTreeSet::new();
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut imports)?
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !["definition", "activated_ability", "triggered_ability"]
                    .contains(&name.as_str())
                    || !functions.insert(name)
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| is_doc_attribute(attr) || attr.path().is_ident("must_use"))
                    || !matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"))
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !function.sig.inputs.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(function.block.stmts.first(),Some(Stmt::Expr(Expr::Struct(value),None)) if value.rest.is_none())
                {
                    return Err("Stone-Seeder module must contain only three direct-literal candidate factories".to_owned());
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    unreachable!()
                };
                if !safe_constructor(&value.path)
                    || !value
                        .fields
                        .iter()
                        .all(|field| safe_value_expr(&field.expr))
                {
                    return Err("Stone-Seeder factory has a nonliteral field expression".to_owned());
                }
            }
            _ => return Err("Stone-Seeder module contains executable top-level code".to_owned()),
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "activated_ability".to_owned(),
            "triggered_ability".to_owned(),
        ])
        || imports != expected_imports
    {
        return Err("Stone-Seeder imports or required factories differ".to_owned());
    }
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}

fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    out: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, out)
        }
        UseTree::Name(name) => {
            let mut full = prefix.to_vec();
            full.push(name.ident.to_string());
            out.insert(full);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, out)?;
            }
            Ok(())
        }
        _ => Err("Stone-Seeder imports must use explicit, unaliased paths".to_owned()),
    }
}

fn safe_constructor(path: &syn::Path) -> bool {
    [
        "CardDefinition",
        "ActivatedAbilityBinding",
        "ActivatedAbility",
        "TriggeredAbilityBinding",
        "TriggeredAbility",
    ]
    .iter()
    .any(|name| path.is_ident(name))
}

fn safe_value_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Array(array) => array.elems.iter().all(safe_value_expr),
        Expr::Call(call) => {
            let Expr::Path(path) = call.func.as_ref() else {
                return false;
            };
            let name = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            ["BTreeSet::from", "ManaCost::new", "ManaCost::with_colors"].contains(&name.as_str())
                && call.args.iter().all(safe_value_expr)
        }
        Expr::Macro(value) if value.mac.path.is_ident("vec") => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            parser
                .parse2(value.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(safe_value_expr))
        }
        Expr::Struct(value) => {
            safe_constructor(&value.path)
                && value.rest.is_none()
                && value
                    .fields
                    .iter()
                    .all(|field| safe_value_expr(&field.expr))
        }
        _ => false,
    }
}

fn card_status(card: &CatalogCard) -> Value {
    if card.collector_number == 184 && card.name == CARD_NAME {
        json!({"collector_number":184,"name":CARD_NAME,"definition_id":CARD_ID,"trace_status":"passed","semantic_status":format!("{:?}",card.semantic_status)})
    } else {
        json!({"collector_number":card.collector_number,"name":card.name,"trace_status":"not_in_pilot","semantic_status":format!("{:?}",card.semantic_status)})
    }
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
fn first_after(
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
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n")
}
