//! Full-game RAV Vinelasher Kudzu landfall authority.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CounterKind, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
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
use syn::{Expr, Item, Meta, ReturnType, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VINELASHER-KUDZU";
const CARD_NAME: &str = "Vinelasher Kudzu";
const ABILITY_ID: &str = "controller-landfall-plus-one-counter";
const FOREST_ID: &str = "RAV-FOREST";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/vinelasher_kudzu.rs";
const MAX_SEED: u64 = 64;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 4_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/vinelasher_kudzu.rs"]
mod candidate_vinelasher_kudzu;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Vinelasher Kudzu report: {error}");
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
            eprintln!("Vinelasher Kudzu PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // Shape validation is the first operation that can inspect the candidate;
    // no candidate factory is invoked before it succeeds.
    validate_candidate_source()?;
    let injected = validate_side_effect_mutation()?;
    validate_candidate_contract()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_reference()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Vinelasher Kudzu full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "base_power_mutation_detected": mutation_detected(seed, Mutation::WrongPower)?,
        "wrong_land_scope_mutation_detected": mutation_detected(seed, Mutation::AnyPlayersLand)?,
        "wrong_counter_effect_mutation_detected": mutation_detected(seed, Mutation::WrongEffect)?,
        "trigger_binding_removal_detected": mutation_detected(seed, Mutation::RemoveTrigger)?,
        "compile_valid_side_effect_source_rejected_before_candidate_calls": injected["side_effect_mutation_compile_valid_by_construction"] == true && injected["side_effect_mutation_rejected"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cost_mutation_detected",
        "base_power_mutation_detected",
        "wrong_land_scope_mutation_detected",
        "wrong_counter_effect_mutation_detected",
        "trigger_binding_removal_detected",
        "compile_valid_side_effect_source_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Vinelasher Kudzu mutation escaped: {key}"));
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
        "card":{"collector_number":189,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true,
            "source_contract":"Original RAV #189 printing: {1}{G} 1/1 Creature — Plant; whenever a land comes into play under its controller's control, put one +1/+1 counter on Vinelasher Kudzu.",
            "oracle_reconciliation":"Current Oracle labels the equivalent controller-land-entry trigger Landfall and uses enters/this creature terminology; event scope, counter, and affected source are unchanged.",
            "subtype_model_limit":"Plant is retained in task source metadata; CardDefinition currently cannot assert creature subtypes.",
            "checked_sections":["305.1-305.2: playing lands and land characteristics","603.2-603.3: triggered ability occurrence and stacking","608.2: resolving abilities","122.1: counters"]
        },
        "source_guard":injected,
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
    AnyPlayersLand,
    WrongEffect,
    RemoveTrigger,
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
    source_mana_cost: Option<ManaCost>,
    cast_payment: Vec<Color>,
    source_resolved: bool,
    source_entered: bool,
    source_zone: Option<Zone>,
    source_base_characteristics: Option<(i32, i32)>,
    final_characteristics: Option<(i32, i32)>,
    final_counter_count: i16,
    land_entries: Vec<LandEntryReceipt>,
    life: [i64; 2],
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
            .any(|card| card.collector_number == 189 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Vinelasher Kudzu".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_contract() -> Result<(), String> {
    let candidate = candidate_vinelasher_kudzu::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vinelasher Kudzu definition is absent from the RAV registry")?;
    let trigger = candidate_vinelasher_kudzu::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Vinelasher Kudzu trigger is absent from the RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(1, [Color::Green])
        || candidate.colors != BTreeSet::from([Color::Green])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Creature])
        || candidate.is_basic_land
        || candidate.power != Some(1)
        || candidate.toughness != Some(1)
        || !candidate.keywords.is_empty()
        || !candidate.effects.is_empty()
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "controlled-land-entry-plus-one-counter",
            ]
        || trigger != registered_trigger
        || trigger.card_definition != CARD_ID
        || trigger.ability.id != ABILITY_ID
        || trigger.ability.condition
            != cardbench_magic_engine::TriggerCondition::ControlledLandEntersBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::AddPlusOneCounterToSource]
    {
        return Err(
            "Vinelasher Kudzu candidate or registry differs from the exact RAV #189 contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn definition_with_mutation(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_vinelasher_kudzu::definition();
    match mutation {
        Mutation::WrongCost => definition.mana_cost = ManaCost::new(2),
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
    let mut trigger = candidate_vinelasher_kudzu::triggered_ability();
    match mutation {
        Mutation::AnyPlayersLand => {
            trigger.ability.condition =
                cardbench_magic_engine::TriggerCondition::LandEntersBattlefield;
        }
        Mutation::WrongEffect => trigger.ability.effects.clear(),
        _ => {}
    }
    Some(trigger)
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let definition = definition_with_mutation(mutation);
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vinelasher Kudzu is absent from the RAV registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(trigger) = trigger_with_mutation(mutation) {
        triggers.push(trigger);
    }
    triggers.extend(rav_attachment_triggered_ability_bindings());
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

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_VINELASHER_KUDZU_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_VINELASHER_KUDZU_SEED must be an unsigned integer")?;
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
        "no Vinelasher Kudzu full-game trace found for seeds 0..={MAX_SEED}: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
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
                "Vinelasher Kudzu {label} deck has {size} cards; expected 60"
            ));
        }
    }
    let config = cardbench_magic_policies::DeckMatchConfig {
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
        Box::new(KudzuPolicy {
            player: PlayerId(0),
            cast_once: false,
            mutation,
        }),
        Box::new(LandPolicy {
            player: PlayerId(1),
        }),
    ];
    let mut events = Vec::new();
    let mut accepted = 0_u32;
    let mut attempts = 0_u32;
    let mut source_base_characteristics = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Vinelasher Kudzu duel exceeded its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Vinelasher Kudzu policy seat missing")?;
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
        for event in &game.event_log[cursor..] {
            if let GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID)
                && let Ok(characteristics) = game.characteristics(*card)
            {
                source_base_characteristics = Some((
                    characteristics
                        .power
                        .ok_or("Vinelasher Kudzu has no power on entry")?,
                    characteristics
                        .toughness
                        .ok_or("Vinelasher Kudzu has no toughness on entry")?,
                ));
            }
        }
        events = game.event_log.clone();
    }
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
    let source_mana_cost = source.and_then(|card| {
        game.card_definition(card)
            .ok()
            .map(|definition| definition.mana_cost.clone())
    });
    let final_characteristics = source
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let final_counter_count = source
        .and_then(|card| game.object(card).ok())
        .and_then(|object| object.counters.get(&CounterKind::PlusOnePlusOne).copied())
        .unwrap_or(0);
    let source_entered_index = source.and_then(|source| events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)));
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
            let after_source =
                source_entered_index.is_some_and(|source_index| event_index > source_index);
            (after_source
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
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events,
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempts.saturating_sub(accepted),
        source,
        source_incarnation,
        source_mana_cost,
        cast_payment,
        source_resolved,
        source_entered,
        source_zone,
        source_base_characteristics,
        final_characteristics,
        final_counter_count,
        land_entries,
        life,
    })
}

struct KudzuPolicy {
    player: PlayerId,
    cast_once: bool,
    mutation: Mutation,
}

struct LandPolicy {
    player: PlayerId,
}

impl CodePolicy for KudzuPolicy {
    fn id(&self) -> &'static str {
        "rav.vinelasher-kudzu-landfall-pilot.v1"
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
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FOREST_ID))
        {
            return PolicyAction::PlayLand { card: land.id };
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
            if let Some(forest) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(FOREST_ID) && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: forest.id,
                    color: Color::Green,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for LandPolicy {
    fn id(&self) -> &'static str {
        "rav.vinelasher-kudzu-opponent-land-drop.v1"
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
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FOREST_ID))
        {
            return PolicyAction::PlayLand { card: land.id };
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
            "Vinelasher Kudzu duel did not reach a clean terminal pilot win with Kudzu alive"
                .to_owned(),
        );
    }
    let source = capture.source.ok_or("Vinelasher Kudzu was not cast")?;
    let source_incarnation = capture
        .source_incarnation
        .ok_or("Vinelasher Kudzu incarnation missing")?;
    if !capture.source_resolved
        || !capture.source_entered
        || capture.source_mana_cost != Some(ManaCost::with_colors(1, [Color::Green]))
        || capture.cast_payment.len() != 2
        || capture
            .cast_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            != 2
        || capture.source_base_characteristics != Some((1, 1))
    {
        return Err("Vinelasher Kudzu did not cast for {1}{G} and enter as a 1/1".to_owned());
    }
    let entry = unique_index(&capture.events, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source
        )
    })
    .ok_or("Vinelasher Kudzu battlefield entry receipt missing")?;
    let payment_event = unique_index(&capture.events, |event| {
        matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                if *card == source && colors.len() == 2 && colors == &[Color::Green, Color::Green]
        )
    })
    .ok_or("Vinelasher Kudzu exact {1}{G} payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == source
        )
    })
    .ok_or("Vinelasher Kudzu SpellCast receipt missing")?;
    let resolved = unique_index(&capture.events, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == source
        )
    })
    .ok_or("Vinelasher Kudzu SpellResolved receipt missing")?;
    if !(payment_event < cast && cast < resolved && resolved < entry) {
        return Err("Vinelasher Kudzu paid cast and entry receipts are out of order".to_owned());
    }
    let pilot_lands: Vec<_> = capture
        .land_entries
        .iter()
        .filter(|land| land.controller == 0 && land.event_index > entry)
        .collect();
    let opponent_lands: Vec<_> = capture
        .land_entries
        .iter()
        .filter(|land| land.controller == 1 && land.event_index > entry)
        .collect();
    if pilot_lands.is_empty() || opponent_lands.is_empty() {
        return Err(
            "trace needs both a post-entry controller land and an opponent land entry".to_owned(),
        );
    }
    let triggers: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::TriggeredAbilityStacked {
                controller: PlayerId(0),
                source: trigger_source,
                source_incarnation: incarnation,
                ability: ABILITY_ID,
            } if *trigger_source == source => Some((index, *incarnation)),
            _ => None,
        })
        .collect();
    let counters: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::CounterPlaced {
                source: counter_source,
                card,
                counter: CounterKind::PlusOnePlusOne,
                amount: 1,
            } if *counter_source == source && *card == source => Some(index),
            _ => None,
        })
        .collect();
    let resolutions: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::AbilityResolved {
                source: resolved_source,
                source_incarnation: incarnation,
                ability: ABILITY_ID,
            } if *resolved_source == source => Some((index, *incarnation)),
            _ => None,
        })
        .collect();
    if triggers.len() != pilot_lands.len()
        || counters.len() != pilot_lands.len()
        || resolutions.len() != pilot_lands.len()
    {
        return Err(format!(
            "each controller land should produce one trigger and one counter, with opponent land ignored: P0 lands={}, P1 lands={}, triggers={}, counters={}, resolves={}",
            pilot_lands.len(),
            opponent_lands.len(),
            triggers.len(),
            counters.len(),
            resolutions.len()
        ));
    }
    for (index, land) in pilot_lands.iter().enumerate() {
        let (trigger_index, trigger_incarnation) = triggers[index];
        let counter_index = counters[index];
        let (resolve_index, resolve_incarnation) = resolutions[index];
        if !(land.event_index < trigger_index
            && trigger_index < counter_index
            && counter_index < resolve_index)
            || trigger_incarnation != source_incarnation
            || resolve_incarnation != source_incarnation
        {
            return Err(
                "land entry, same-incarnation trigger, source counter, and resolution order differ"
                    .to_owned(),
            );
        }
    }
    let expected_counters =
        i16::try_from(pilot_lands.len()).map_err(|_| "too many Kudzu land entries")?;
    if capture.final_counter_count != expected_counters
        || capture.final_characteristics
            != Some((
                i32::from(1 + expected_counters),
                i32::from(1 + expected_counters),
            ))
    {
        return Err("Vinelasher Kudzu final counters and derived power/toughness differ from its controller land entries".to_owned());
    }
    let final_resolution = resolutions
        .last()
        .map(|item| item.0)
        .ok_or("last Kudzu trigger resolution missing")?;
    let attack = event_index_after(&capture.events, final_resolution, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source)
    )).ok_or("Vinelasher Kudzu did not attack after a landfall trigger resolved")?;
    let damage = event_index_after(&capture.events, attack, |event| {
        matches!(event,
            GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount }
                if *damage_source == source && *amount > 0
        )
    })
    .ok_or("Vinelasher Kudzu dealt no combat damage to the opponent")?;
    let terminal = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal Player 0 GameEnded receipt missing")?;
    if !(final_resolution < attack && attack < damage && damage < terminal) {
        return Err("Kudzu landfall, combat, and terminal-win event order differs".to_owned());
    }
    let mut required = vec![payment_event, cast, resolved, entry];
    for (index, land) in pilot_lands.iter().enumerate() {
        required.extend([
            land.event_index,
            triggers[index].0,
            counters[index],
            resolutions[index].0,
        ]);
    }
    required.extend(opponent_lands.iter().map(|land| land.event_index));
    required.extend([attack, damage, terminal]);
    required.sort_unstable();
    let required_set: BTreeSet<_> = required.iter().copied().collect();
    let events = project_events(&capture.events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| required_set.contains(&index).then_some(event))
        .collect::<Vec<_>>();
    Ok(json!({
        "id":"rav-vinelasher-kudzu-controller-landfall-full-game",
        "seed":seed,
        "decks":["rav-vinelasher-kudzu-pilot-60","rav-basic-forest-opponent-60"],
        "policies":["rav.vinelasher-kudzu-landfall-pilot.v1","rav.vinelasher-kudzu-opponent-land-drop.v1"],
        "terminal":"Winner(PlayerId(0))",
        "winner_seat":0,
        "turns":capture.turns,
        "accepted_moves":capture.accepted_moves,
        "rejected_moves":capture.rejected_moves,
        "mechanics":{
            "object":source.0,
            "definition_id":CARD_ID,
            "mana_cost":"{1}{G}",
            "paid_mana_colors":capture.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "source_incarnation":source_incarnation,
            "base_power":1,
            "base_toughness":1,
            "pilot_land_entries_after_entry":pilot_lands.iter().map(|land|land.card.0).collect::<Vec<_>>(),
            "opponent_land_entries_after_entry":opponent_lands.iter().map(|land|land.card.0).collect::<Vec<_>>(),
            "trigger_count":triggers.len(),
            "counter_receipt_count":counters.len(),
            "counter_count_at_terminal":capture.final_counter_count,
            "derived_power_at_terminal":capture.final_characteristics.map(|characteristics|characteristics.0),
            "derived_toughness_at_terminal":capture.final_characteristics.map(|characteristics|characteristics.1),
            "life_at_terminal":capture.life,
            "opponent_land_triggered_source":false,
            "terminal_pilot_win":true,
        },
        "ordered_event_sequence":required,
        "events":events,
        "passed":true
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Vinelasher Kudzu source: {error}"))?;
    validate_source_shape(&source)
}

fn validate_source_shape(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|_| "Vinelasher Kudzu source is not valid Rust")?;
    if !file.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
    }) {
        return Err("Vinelasher Kudzu has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
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
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                imports.extend(flattened_use_paths(item).ok_or("unsupported candidate import")?);
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let return_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => return Err("candidate exposes an unsupported function".to_owned()),
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || !matches!(&function.sig.output, ReturnType::Type(_, output) if matches!(&**output, syn::Type::Path(path) if path.path.is_ident(return_type)))
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attribute| {
                        (attribute.path().is_ident("doc")
                            && matches!(attribute.meta, Meta::NameValue(_)))
                            || (attribute.path().is_ident("must_use")
                                && matches!(attribute.meta, Meta::Path(_)))
                    })
                {
                    return Err("candidate function signature or attributes are outside the source contract".to_owned());
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "candidate function must directly return a struct literal".to_owned()
                    );
                };
                if literal
                    .path
                    .segments
                    .last()
                    .is_none_or(|segment| segment.ident != return_type)
                    || literal.rest.is_some()
                    || !declarative_expr(&Expr::Struct(literal.clone()))
                {
                    return Err("candidate literal contains an unsupported expression".to_owned());
                }
                let expected_fields: BTreeSet<&str> = if name == "definition" {
                    [
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
                    ]
                    .into_iter()
                    .collect()
                } else {
                    ["card_definition", "ability"].into_iter().collect()
                };
                let fields: BTreeSet<String> = literal
                    .fields
                    .iter()
                    .filter_map(|field| match &field.member {
                        syn::Member::Named(name) => Some(name.to_string()),
                        _ => None,
                    })
                    .collect();
                if fields.len() != literal.fields.len()
                    || fields.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields
                {
                    return Err(
                        "candidate literal field set differs from the pinned schema".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "candidate source permits only imports and the two factories".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
    {
        return Err(
            "candidate module import/function shape differs from the pinned schema".to_owned(),
        );
    }
    Ok(())
}

fn declarative_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            matches!(
                value
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .as_deref(),
                Some("CardDefinition" | "TriggeredAbilityBinding" | "TriggeredAbility")
            ) && value.attrs.is_empty()
                && value.rest.is_none()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [owner, method] if (owner == "BTreeSet" && (method == "from" || method == "new")) || (owner == "ManaCost" && (method == "new" || method == "with_colors")))
                        || matches!(names.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut output = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut output).then_some(output)
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, output: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, output)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            output.push(next);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, output)),
        _ => false,
    }
}

fn validate_side_effect_mutation() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let function = source
        .find("fn definition")
        .ok_or("could not locate definition() for source mutation")?;
    let open = source[function..]
        .find('{')
        .map(|offset| function + offset + 1)
        .ok_or("could not locate definition() body for source mutation")?;
    let injected = format!(
        "{}\n    let _ = std::fs::write(\"/tmp/cardbench-vinelasher-source-guard-executed\", b\"candidate ran\");{}",
        &source[..open],
        &source[open..]
    );
    if injected == source {
        return Err("could not create the side-effecting Rust source mutation".to_owned());
    }
    syn::parse_file(&injected).map_err(|_| "side-effect mutation must remain parse-valid Rust")?;
    if validate_source_shape(&injected).is_ok() {
        return Err("candidate source guard accepted executable side-effect code".to_owned());
    }
    Ok(json!({
        "normalized_ast_schema_checked_before_candidate_calls":true,
        "side_effect_mutation_parse_valid":true,
        "side_effect_mutation_compile_valid_by_construction":true,
        "side_effect_mutation_rejected":true,
        "candidate_calls_before_guard":0,
        "rejection_stage":"source-shape-and-declarative-ast-before-candidate-function-invocation"
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status":if passed { "passed" } else { "not_qualified" },"credited":false,
        "qualification":if passed { "local_reference_pass_uncredited; independent review pending" } else { "not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
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
