//! Full-game diagnostic authority for the RAV Hunted Troll task.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, Color, CreatureSubtype, DecisionKind,
    DecisionSelection, DeckList, Effect, Game, GameEvent, GameView, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TokenSpec, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, load_constructed_decks,
    rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
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
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs";
const DECK_P0: &str = "rav_golgari_midrange";
const DECK_P1: &str = "rav_golgari_midrange";
const CANDIDATE_ID: &str = "RAV-HUNTED-TROLL";
const BRAINSPOIL_ID: &str = "RAV-BRAINSPOIL";
const ETB_ABILITY: &str = "etb-opponent-faeries";
const REGEN_ABILITY: &str = "self-regeneration";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/hunted_troll.rs"]
mod candidate_hunted_troll;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Hunted Troll result: {error}");
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
            eprintln!("Hunted Troll authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }

    let seed = std::env::var("CARDBENCH_HUNTED_TROLL_SEED")
        .map_or(0, |value| value.parse().unwrap_or(u64::MAX));
    if seed == u64::MAX {
        return Err("CARDBENCH_HUNTED_TROLL_SEED must be unsigned".to_owned());
    }
    let etb = run_match(seed, Scenario::EtbOnly, Mutation::None)?;
    let etb_trace = validate_trace(&etb, Scenario::EtbOnly, seed)?;
    let regeneration = run_match(seed, Scenario::Regeneration, Mutation::None)?;
    let regeneration_trace = validate_trace(&regeneration, Scenario::Regeneration, seed)?;
    for (capture, scenario) in [
        (&etb, Scenario::EtbOnly),
        (&regeneration, Scenario::Regeneration),
    ] {
        let replay = run_match(seed, scenario, Mutation::None)?;
        if capture.events != replay.events
            || capture.terminal != replay.terminal
            || capture.winner != replay.winner
            || capture.troll != replay.troll
            || capture.brainspoil != replay.brainspoil
            || capture.faeries != replay.faeries
            || project_events(&capture.events) != project_events(&replay.events)
        {
            return Err("Hunted Troll full-game replay was not deterministic".to_owned());
        }
    }
    let mutation_sensitivity = json!({
        "body_mutation_detected_in_game": mutation_detected(seed, Scenario::Regeneration, Mutation::Power, "resolved Hunted Troll is not an 8/4"),
        "etb_trigger_removal_detected_in_game": mutation_detected(seed, Scenario::Regeneration, Mutation::NoEtb, "Hunted Troll ETB did not select the opponent or preserve its incarnation"),
        "wrong_target_shape_detected_in_game": mutation_detected(seed, Scenario::Regeneration, Mutation::WrongTarget, "Hunted Troll ETB did not select the opponent"),
        "wrong_token_detected_in_game": mutation_detected(seed, Scenario::Regeneration, Mutation::WrongToken, "opponent did not receive four 1/1 blue flying Faeries"),
        "regeneration_cost_mutation_detected_in_game": mutation_detected(seed, Scenario::Regeneration, Mutation::WrongRegenerationMana, "Hunted Troll green regeneration ability was not paid and resolved"),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["body_mutation_detected_in_game"] != true
        || mutation_sensitivity["etb_trigger_removal_detected_in_game"] != true
        || mutation_sensitivity["wrong_target_shape_detected_in_game"] != true
        || mutation_sensitivity["wrong_token_detected_in_game"] != true
        || mutation_sensitivity["regeneration_cost_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Hunted Troll semantic mutation escaped the trace oracle: {mutation_sensitivity}"
        ));
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
        "card": {"collector_number": 170, "definition_id": CANDIDATE_ID, "name": "Hunted Troll"},
        "candidate": {"kind": candidate_kind(), "card_definition": CANDIDATE_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "110 and 111: permanents and token characteristics",
                "117: timing and priority",
                "302.1 and 601.2: creature spells and casting",
                "603.2 and 603.3: ETB trigger and target selection",
                "608.2: resolving spells and abilities",
                "701.15: regeneration replacement effect",
                "614.8: regeneration shield replaces destruction"
            ],
            "source_contract": "RAV printed green 8/4 Hunted Troll; opponent-targeted ETB creates four blue 1/1 flying Faeries; {G}: regenerate Hunted Troll",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [etb_trace, regeneration_trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hunted Troll source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Hunted Troll source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Hunted Troll source has an unsupported module attribute".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(item.vis, Visibility::Restricted(_))
                    && item.sig.inputs.is_empty() =>
            {
                functions.insert(item.sig.ident.to_string());
            }
            _ => {
                return Err("Hunted Troll source contains an unsupported top-level item".to_owned());
            }
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "activated_ability".to_owned(),
            "triggered_ability".to_owned(),
        ])
    {
        return Err("Hunted Troll must expose only definition(), activated_ability(), and triggered_ability()".to_owned());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CANDIDATE_ID)
        .ok_or("Hunted Troll definition is missing from the RAV registry")?;
    if candidate_hunted_troll::definition() != definition {
        return Err(
            "Hunted Troll candidate definition differs from the executable RAV definition"
                .to_owned(),
        );
    }
    let activated = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == CANDIDATE_ID && binding.ability.id == REGEN_ABILITY
        })
        .ok_or("Hunted Troll regeneration binding is missing from the RAV registry")?;
    if candidate_hunted_troll::activated_ability() != activated {
        return Err(
            "Hunted Troll candidate regeneration binding differs from the executable RAV binding"
                .to_owned(),
        );
    }
    let triggered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == CANDIDATE_ID && binding.ability.id == ETB_ABILITY
        })
        .ok_or("Hunted Troll ETB binding is missing from the RAV registry")?;
    if candidate_hunted_troll::triggered_ability() != triggered {
        return Err(
            "Hunted Troll candidate ETB binding differs from the executable RAV binding".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    EtbOnly,
    Regeneration,
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    Power,
    NoEtb,
    WrongTarget,
    WrongToken,
    WrongRegenerationMana,
}

fn mutation_detected(seed: u64, scenario: Scenario, mutation: Mutation, expected: &str) -> bool {
    matches!(run_match(seed, scenario, mutation).and_then(|capture| validate_trace(&capture, scenario, seed).map(|_| ())), Err(error) if error.contains(expected))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    troll: Option<ObjectId>,
    brainspoil: Option<ObjectId>,
    faeries: Vec<ObjectId>,
    troll_stats: Option<(i32, i32)>,
    faerie_stats: Vec<(
        Vec<Color>,
        Vec<CardType>,
        Vec<CreatureSubtype>,
        i32,
        i32,
        Vec<String>,
        PlayerId,
    )>,
    etb_target: Option<PlayerId>,
    brainspoil_target: Option<ObjectId>,
    etb_incarnation: Option<u64>,
    etb_resolved_incarnation: Option<u64>,
    regen_payment: Option<(u8, Vec<Color>)>,
    regen_incarnation: Option<u64>,
    regen_resolved_incarnation: Option<u64>,
    shield_created: bool,
    shield_used: bool,
    troll_tapped_after_shield: bool,
    troll_on_battlefield_after_shield: bool,
}

fn task_deck(base: &DeckList) -> Result<DeckList, String> {
    let mut deck = base.clone();
    let slot = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-SEWERDREG")
        .ok_or("Golgari deck has no four-card Sewerdreg slot")?;
    if slot.count != 4 {
        return Err("Hunted Troll replacement slot is not a four-of".to_owned());
    }
    slot.card = CANDIDATE_ID.to_owned();
    let total: usize = deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if total != 60 {
        return Err(format!(
            "Hunted Troll task deck has {total} cards, expected 60"
        ));
    }
    Ok(deck)
}

fn task_opponent_deck(base: &DeckList) -> Result<DeckList, String> {
    let mut deck = base.clone();
    let slot = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-PUTREFY")
        .ok_or("Golgari opponent deck has no four-card Putrefy slot")?;
    if slot.count != 4 {
        return Err("Hunted Troll opponent removal replacement is not a four-of".to_owned());
    }
    slot.card = BRAINSPOIL_ID.to_owned();
    Ok(deck)
}

fn run_match(seed: u64, scenario: Scenario, mutation: Mutation) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let source0 = decks
        .iter()
        .find(|deck| deck.id == DECK_P0)
        .ok_or("Golgari deck missing")?;
    let source1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Golgari deck missing")?;
    let deck0 = task_deck(&source0.deck)?;
    let deck1 = task_opponent_deck(&source1.deck)?;
    if deck1
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
    {
        return Err("Hunted Troll opponent deck is not 60 cards".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 60,
        max_policy_moves: 8_000,
        ..DeckMatchConfig::default()
    };
    let mut game = new_rav_game(candidate_hunted_troll::definition(), mutation)?;
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
    let mut policies: Vec<Box<dyn CodePolicy>> = vec![
        Box::new(HuntedPolicy::new(PlayerId(0), scenario, mutation)),
        Box::new(HuntedPolicy::new(PlayerId(1), scenario, mutation)),
    ];
    let mut result = CapturedMatch {
        events: Vec::new(),
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        troll: None,
        brainspoil: None,
        faeries: Vec::new(),
        troll_stats: None,
        faerie_stats: Vec::new(),
        etb_target: None,
        brainspoil_target: None,
        etb_incarnation: None,
        etb_resolved_incarnation: None,
        regen_payment: None,
        regen_incarnation: None,
        regen_resolved_incarnation: None,
        shield_created: false,
        shield_used: false,
        troll_tapped_after_shield: false,
        troll_on_battlefield_after_shield: false,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || result.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Hunted Troll game hit a run bound in {scenario:?} / {mutation:?}: turn={}, step={:?}, troll={:?}, Brainspoil={:?}, tokens={}, life=[{}, {}], moves={}",
                game.turn,
                game.step,
                result.troll,
                result.brainspoil,
                result.faeries.len(),
                game.players[0].life,
                game.players[1].life,
                result.accepted_moves
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
        let previous = game.event_log.len();
        let chosen_etb_target = matches!(&action, PolicyAction::ChooseTriggeredAbilityTargets { source, ability, targets, .. } if Some(*source) == result.troll && *ability == ETB_ABILITY && targets == &[Target::Player(PlayerId(1))]);
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("Hunted Troll policy move rejected: {error}"))?;
        result.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if chosen_etb_target {
            let stack = game
                .stack
                .last()
                .ok_or("Hunted Troll ETB trigger did not enter the stack")?;
            result.etb_target = match stack.targets.first() {
                Some(Target::Player(seat)) => Some(*seat),
                _ => None,
            };
            result.etb_incarnation = Some(stack.source_incarnation);
            if result.etb_target != Some(PlayerId(1))
                || game.zone_of(stack.card) != Some(Zone::Battlefield)
            {
                return Err(
                    "Hunted Troll ETB target was not the opponent while Troll was live".to_owned(),
                );
            }
        }
        for event in &game.event_log[previous..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    result.troll = Some(*card);
                    if colors.len() != 4
                        || colors
                            .iter()
                            .filter(|color| **color == Color::Green)
                            .count()
                            < 2
                    {
                        return Err(
                            "Hunted Troll was not paid with two Green and two generic mana"
                                .to_owned(),
                        );
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    result.troll = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == BRAINSPOIL_ID) =>
                {
                    result.brainspoil = Some(*card);
                    if colors.len() != 5
                        || colors
                            .iter()
                            .filter(|color| **color == Color::Black)
                            .count()
                            < 2
                    {
                        return Err(
                            "Brainspoil payment did not include two Black and three generic mana"
                                .to_owned(),
                        );
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == BRAINSPOIL_ID) =>
                {
                    result.brainspoil = Some(*card);
                    let stack = game
                        .stack
                        .last()
                        .ok_or("Brainspoil did not enter the stack")?;
                    result.brainspoil_target = match stack.targets.first() {
                        Some(Target::Permanent(target)) => Some(*target),
                        _ => None,
                    };
                }
                GameEvent::SpellResolved { card } if Some(*card) == result.troll => {
                    let c = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    result.troll_stats = Some((
                        c.power.ok_or("Hunted Troll has no power")?,
                        c.toughness.ok_or("Hunted Troll has no toughness")?,
                    ));
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } if Some(*source) == result.troll && *ability == ETB_ABILITY => {
                    result.etb_incarnation = Some(*source_incarnation)
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability,
                } if Some(*source) == result.troll && *ability == ETB_ABILITY => {
                    result.etb_resolved_incarnation = Some(*source_incarnation)
                }
                GameEvent::TokenCreated { player, token }
                    if *player == PlayerId(1) || *player == PlayerId(0) =>
                {
                    if game
                        .object(*token)
                        .is_ok_and(|object| object.token.is_some())
                    {
                        result.faeries.push(*token);
                    }
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source,
                    ability,
                    mana_cost,
                } if Some(*source) == result.troll && *ability == REGEN_ABILITY => {
                    result.regen_payment = Some((mana_cost.generic, mana_cost.colored.clone()));
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    source_incarnation,
                    ability,
                    ..
                } if Some(*source) == result.troll && *ability == REGEN_ABILITY => {
                    result.regen_incarnation = Some(*source_incarnation)
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability,
                } if Some(*source) == result.troll && *ability == REGEN_ABILITY => {
                    result.regen_resolved_incarnation = Some(*source_incarnation)
                }
                GameEvent::RegenerationShieldCreated { source, target }
                    if Some(*source) == result.troll && Some(*target) == result.troll =>
                {
                    result.shield_created = true
                }
                GameEvent::RegenerationShieldUsed { source, target }
                    if Some(*source) == result.troll && Some(*target) == result.troll =>
                {
                    result.shield_used = true;
                    result.troll_tapped_after_shield =
                        game.object(*target).is_ok_and(|object| object.tapped);
                    result.troll_on_battlefield_after_shield =
                        game.zone_of(*target) == Some(Zone::Battlefield);
                }
                _ => {}
            }
        }
        if result.faeries.len() >= 4 && result.faerie_stats.len() != 4 {
            result.faeries.sort();
            result.faeries.dedup();
            result.faerie_stats = result
                .faeries
                .iter()
                .take(4)
                .map(|token| -> Result<_, String> {
                    let characteristics = game.characteristics(*token).map_err(|error| {
                        format!("token {} is not live while capturing: {error}; zone={:?}; captured={:?}", token.0, game.zone_of(*token), result.faeries.iter().map(|item| item.0).collect::<Vec<_>>())
                    })?;
                    let controller = game.object(*token).map_err(|error| error.to_string())?.controller;
                    Ok((
                        characteristics.colors.into_iter().collect(),
                        characteristics.card_types.into_iter().collect(),
                        characteristics.creature_subtypes.into_iter().collect(),
                        characteristics.power.unwrap_or_default(),
                        characteristics.toughness.unwrap_or_default(),
                        characteristics
                            .keywords
                            .iter()
                            .map(|keyword| format!("{keyword:?}"))
                            .collect(),
                        controller,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
        }
    }
    result.events = game.event_log.clone();
    result.terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |p| format!("Winner(PlayerId({}))", p.0),
    );
    result.winner = game.winner().map(|p| p.0);
    result.turns = game.turn;
    Ok(result)
}

fn new_rav_game(mut definition: CardDefinition, mutation: Mutation) -> Result<Game, String> {
    if matches!(mutation, Mutation::Power) {
        definition.power = Some(7);
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CANDIDATE_ID)
        .ok_or("Hunted Troll definition missing from RAV registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| {
        !(binding.card_definition == CANDIDATE_ID && binding.ability.id == REGEN_ABILITY)
    });
    let mut regen = candidate_hunted_troll::activated_ability();
    if matches!(mutation, Mutation::WrongRegenerationMana) {
        regen.ability.mana_cost = cardbench_magic_engine::ManaCost::with_colors(0, [Color::Black]);
    }
    abilities.push(regen);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| {
        !(binding.card_definition == CANDIDATE_ID && binding.ability.id == ETB_ABILITY)
    });
    if !matches!(mutation, Mutation::NoEtb) {
        let mut etb = candidate_hunted_troll::triggered_ability();
        match mutation {
            Mutation::WrongTarget => {
                etb.ability.targets = vec![cardbench_magic_engine::TargetRequirement::Player];
                etb.ability.effects = vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::blue_faerie(),
                    count: 4,
                }];
            }
            Mutation::WrongToken => {
                etb.ability.effects = vec![Effect::CreateTokenForTargetOpponent {
                    token: TokenSpec::saproling(),
                    count: 4,
                }]
            }
            _ => {}
        }
        triggers.push(etb);
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
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

struct HuntedPolicy {
    player: PlayerId,
    scenario: Scenario,
    mutation: Mutation,
    inner: RavCatalogPolicy,
    troll_cast: bool,
    brainspoil_cast: bool,
    regen_activated: bool,
}

impl HuntedPolicy {
    fn new(player: PlayerId, scenario: Scenario, mutation: Mutation) -> Self {
        Self {
            player,
            scenario,
            mutation,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            troll_cast: false,
            brainspoil_cast: false,
            regen_activated: false,
        }
    }

    fn play_basic_land(&self, view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| matches!(card.definition, Some("RAV-FOREST" | "RAV-SWAMP")))?;
        Some(PolicyAction::PlayLand { card: card.id })
    }

    fn target_decision(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ETB_ABILITY {
            return None;
        }
        let target = if matches!(self.mutation, Mutation::WrongTarget) {
            PlayerId(0)
        } else {
            PlayerId(1)
        };
        if !choice
            .target_options
            .first()?
            .contains(&Target::Player(target))
        {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![Target::Player(target)],
        })
    }

    fn troll_cast_action(&self, view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CANDIDATE_ID))?;
        let green = view.mana_pool.amount(Color::Green);
        if green < 2 || view.mana_pool.total() < 4 {
            return None;
        }
        let leave_color = if matches!(self.mutation, Mutation::WrongRegenerationMana) {
            Color::Black
        } else {
            Color::Green
        };
        if !view.own_battlefield.iter().any(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && land.mana_colors.contains(&leave_color)
        }) {
            return None;
        }
        let generic = generic_payment(view, 2, 2)?;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic,
                hybrid: vec![],
            },
        })
    }

    fn regenerate_action(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.scenario != Scenario::Regeneration
            || self.regen_activated
            || !view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(BRAINSPOIL_ID))
        {
            return None;
        }
        let troll = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CANDIDATE_ID))?;
        let color = if matches!(self.mutation, Mutation::WrongRegenerationMana) {
            Color::Black
        } else {
            Color::Green
        };
        if view.mana_pool.amount(color) == 0 {
            let land = view.own_battlefield.iter().find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
            })?;
            return Some(PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            });
        }
        self.regen_activated = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: troll.id,
                ability_id: REGEN_ABILITY,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }

    fn brainspoil_action(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(1)
            || self.scenario != Scenario::Regeneration
            || self.brainspoil_cast
            || view.stack_depth != 0
        {
            return None;
        }
        let troll = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(CANDIDATE_ID))?;
        let spell = view
            .hand
            .iter()
            .find(|card| card.definition == Some(BRAINSPOIL_ID))?;
        let generic = generic_payment_excluding(view, 3, Color::Black, 2);
        if view.mana_pool.amount(Color::Black) >= 2
            && view.mana_pool.total() >= 5
            && generic.is_some()
        {
            self.brainspoil_cast = true;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: spell.id,
                    targets: vec![Target::Permanent(troll.id)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: generic?,
                    hybrid: vec![],
                },
            });
        }
        let color = Color::Black;
        let land = view.own_battlefield.iter().find(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && land.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}

impl CodePolicy for HuntedPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.hunted-troll-controller.v1"
        } else {
            "rav.hunted-troll-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(0)
                && view.stack_depth == 0
                && (self.scenario != Scenario::Regeneration || self.regen_activated)
            {
                if let Some(troll) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CANDIDATE_ID) && card.can_attack)
                {
                    return PolicyAction::DeclareAttackers {
                        attackers: vec![troll.id],
                    };
                }
            }
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
        if view.player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && let Some(land) = self.play_basic_land(view)
            {
                return land;
            }
            if self.player == PlayerId(0) && !self.troll_cast {
                if let Some(action) = self.troll_cast_action(view) {
                    self.troll_cast = true;
                    return action;
                }
                if view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CANDIDATE_ID))
                {
                    if let Some(action) =
                        mana_production_action(view, Color::Green, 2, 4, Color::Green)
                    {
                        return action;
                    }
                    return PolicyAction::PassPriority;
                }
            }
            if self.player == PlayerId(1) {
                if let Some(action) = self.brainspoil_action(view) {
                    return action;
                }
                if self.scenario == Scenario::Regeneration
                    && view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(BRAINSPOIL_ID))
                    && view
                        .opponent_battlefield
                        .iter()
                        .any(|card| card.definition == Some(CANDIDATE_ID))
                {
                    return PolicyAction::PassPriority;
                }
            }
        }
        if let Some(action) = self.target_decision(view) {
            return action;
        }
        if let Some(action) = self.regenerate_action(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player == PlayerId(1)
            && let Some(decision) = view.pending_decision.as_ref()
            && decision.kind == DecisionKind::CleanupDiscard
            && let Some(card) = decision
                .candidates
                .iter()
                .find(|card| card.definition != Some(BRAINSPOIL_ID))
        {
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(vec![card.id]),
            });
        }
        self.target_decision(view)
            .or_else(|| self.inner.propose_pending_decision(view))
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn generic_payment(
    view: &GameView,
    generic_needed: usize,
    colored_green: u8,
) -> Option<Vec<Color>> {
    let mut counts = [
        (Color::Black, view.mana_pool.amount(Color::Black)),
        (
            Color::Green,
            view.mana_pool
                .amount(Color::Green)
                .checked_sub(colored_green)?,
        ),
        (Color::White, view.mana_pool.amount(Color::White)),
        (Color::Blue, view.mana_pool.amount(Color::Blue)),
        (Color::Red, view.mana_pool.amount(Color::Red)),
        (Color::Colorless, view.mana_pool.amount(Color::Colorless)),
    ];
    let mut payment = Vec::new();
    for (color, count) in &mut counts {
        while *count > 0 && payment.len() < generic_needed {
            payment.push(*color);
            *count -= 1;
        }
    }
    (payment.len() == generic_needed).then_some(payment)
}

fn generic_payment_excluding(
    view: &GameView,
    generic_needed: usize,
    required_color: Color,
    required_amount: u8,
) -> Option<Vec<Color>> {
    let mut counts = [
        Color::Black,
        Color::Green,
        Color::White,
        Color::Blue,
        Color::Red,
        Color::Colorless,
    ]
    .map(|color| {
        (
            color,
            view.mana_pool
                .amount(color)
                .checked_sub(if color == required_color {
                    required_amount
                } else {
                    0
                }),
        )
    });
    let mut payment = Vec::new();
    for (color, count) in &mut counts {
        let mut count = (*count)?;
        while count > 0 && payment.len() < generic_needed {
            payment.push(*color);
            count -= 1;
        }
    }
    (payment.len() == generic_needed).then_some(payment)
}

fn mana_production_action(
    view: &GameView,
    required_color: Color,
    required_amount: u8,
    total_amount: u8,
    preserve_color: Color,
) -> Option<PolicyAction> {
    let pool_color = view.mana_pool.amount(required_color);
    let total = view.mana_pool.total();
    let preserve_sources = view
        .own_battlefield
        .iter()
        .filter(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && land.mana_colors.contains(&preserve_color)
        })
        .count();
    let need_color = pool_color < required_amount;
    let color = if need_color {
        required_color
    } else {
        Color::Black
    };
    let source = view.own_battlefield.iter().find(|land| {
        !land.tapped
            && land.card_types.contains(&CardType::Land)
            && land.mana_colors.contains(&color)
            && !(color == preserve_color && preserve_sources <= 1)
    });
    if let Some(land) = source {
        return Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        });
    }
    if total < total_amount {
        for color in [Color::Black, Color::Green] {
            let sources = view
                .own_battlefield
                .iter()
                .filter(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && land.mana_colors.contains(&color)
                })
                .count();
            if let Some(land) = view.own_battlefield.iter().find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
                    && !(color == preserve_color && sources <= 1)
            }) {
                return Some(PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                });
            }
        }
    }
    None
}

fn validate_trace(capture: &CapturedMatch, scenario: Scenario, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.accepted_moves == 0
    {
        return Err("Hunted Troll full game did not reach the expected terminal winner".to_owned());
    }
    let troll = capture.troll.ok_or("Hunted Troll was not cast")?;
    if capture.troll_stats != Some((8, 4)) {
        return Err("resolved Hunted Troll is not an 8/4".to_owned());
    }
    if capture.etb_target != Some(PlayerId(1))
        || capture.etb_incarnation.is_none()
        || capture.etb_incarnation != capture.etb_resolved_incarnation
    {
        return Err(
            "Hunted Troll ETB did not select the opponent or preserve its incarnation".to_owned(),
        );
    }
    if capture.faeries.len() != 4
        || capture.faerie_stats.len() != 4
        || capture.faerie_stats.iter().any(
            |(colors, types, subtypes, power, toughness, keywords, controller)| {
                colors != &[Color::Blue]
                    || types != &[CardType::Creature]
                    || subtypes != &[CreatureSubtype::Faerie]
                    || *power != 1
                    || *toughness != 1
                    || !keywords.iter().any(|keyword| keyword == "Flying")
                    || *controller != PlayerId(1)
            },
        )
    {
        return Err("opponent did not receive four 1/1 blue flying Faeries".to_owned());
    }
    if matches!(scenario, Scenario::Regeneration) {
        if capture.brainspoil.is_none() || capture.brainspoil_target != Some(troll) {
            return Err(format!(
                "Brainspoil did not target the Hunted Troll: brainspoil={:?}, target={:?}, troll={troll:?}",
                capture.brainspoil, capture.brainspoil_target
            ));
        }
        if capture.regen_payment != Some((0, vec![Color::Green]))
            || capture.regen_incarnation.is_none()
            || capture.regen_incarnation != capture.regen_resolved_incarnation
            || !capture.shield_created
            || !capture.shield_used
            || !capture.troll_tapped_after_shield
            || !capture.troll_on_battlefield_after_shield
        {
            return Err(
                "Hunted Troll green regeneration ability was not paid and resolved".to_owned(),
            );
        }
    }
    let projection = project_events(&capture.events);
    let events = select_events(&projection, capture, scenario)?;
    Ok(json!({
        "id": if scenario == Scenario::EtbOnly {"rav-hunted-troll-targeted-etb-faeries"} else {"rav-hunted-troll-targeted-etb-and-regeneration"},
        "seed":seed,"decks":[format!("{DECK_P0}+4-hunted-troll"),format!("{DECK_P1}+4-brainspoil")],
        "policies":["rav.hunted-troll-controller.v1","rav.hunted-troll-opponent.v1"],
        "terminal":capture.terminal,"winner_seat":capture.winner,"turns":capture.turns,"accepted_moves":capture.accepted_moves,"rejected_moves":0,
        "troll_object":troll.0,"brainspoil_object":capture.brainspoil.map(|card|card.0),"brainspoil_target_object":capture.brainspoil_target.map(|card|card.0),"troll_characteristics":{"power":8,"toughness":4},
        "etb":{"target_seat":capture.etb_target.map(|seat|seat.0),"source_incarnation":capture.etb_incarnation,"resolved_incarnation":capture.etb_resolved_incarnation,"tokens":capture.faeries.iter().zip(&capture.faerie_stats).map(|(token,(colors,types,subtypes,power,toughness,keywords,controller))|json!({"object":token.0,"controller_seat":controller.0,"colors":colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"card_types":types.iter().map(|card_type|format!("{card_type:?}")).collect::<Vec<_>>(),"subtypes":subtypes.iter().map(|subtype|format!("{subtype:?}")).collect::<Vec<_>>(),"power":power,"toughness":toughness,"keywords":keywords})).collect::<Vec<_>>()},
        "regeneration": if scenario == Scenario::Regeneration {json!({"mana_cost":{"generic":capture.regen_payment.as_ref().map(|x|x.0),"colored":capture.regen_payment.as_ref().map(|x|x.1.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>())},"source_incarnation":capture.regen_incarnation,"resolved_incarnation":capture.regen_resolved_incarnation,"shield_created":capture.shield_created,"shield_used":capture.shield_used,"permanent_tapped":capture.troll_tapped_after_shield,"still_on_battlefield":capture.troll_on_battlefield_after_shield})} else {Value::Null},
        "events":events,"passed":true
    }))
}

fn select_events(
    projected: &[ProjectedEvent],
    capture: &CapturedMatch,
    scenario: Scenario,
) -> Result<Vec<ProjectedEvent>, String> {
    let troll = capture.troll.ok_or("Hunted Troll identity missing")?;
    let mut selected = Vec::new();
    let mut take =
        |predicate: &dyn Fn(&ProjectedEvent) -> bool, label: &str| -> Result<(), String> {
            let index = projected
                .iter()
                .position(predicate)
                .ok_or_else(|| format!("typed {label} event is missing"))?;
            let event = projected[index].clone();
            if selected
                .last()
                .is_some_and(|previous: &ProjectedEvent| previous.sequence >= event.sequence)
            {
                return Err(format!("typed {label} event order differs"));
            }
            selected.push(event);
            Ok(())
        };
    take(
        &|e| {
            e.kind == "SpellManaPaid"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                && e.facts.seats == [cardbench_magic_protocol::SeatId(0)]
        },
        "Hunted Troll payment",
    )?;
    take(
        &|e| {
            e.kind == "SpellCast"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                && e.facts.seats == [cardbench_magic_protocol::SeatId(0)]
        },
        "Hunted Troll cast",
    )?;
    take(
        &|e| {
            e.kind == "SpellResolved"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
        },
        "Hunted Troll resolution",
    )?;
    take(
        &|e| {
            e.kind == "CardMoved"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                && e.facts.zone.as_deref() == Some("Battlefield")
        },
        "Hunted Troll entry",
    )?;
    take(
        &|e| {
            e.kind == "TriggeredAbilityStacked"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                && e.facts.ability.as_deref() == Some(ETB_ABILITY)
        },
        "Hunted Troll ETB trigger",
    )?;
    for token in &capture.faeries {
        take(
            &|e| {
                e.kind == "TokenCreated"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(token.0)]
                    && e.facts.seats == [cardbench_magic_protocol::SeatId(1)]
            },
            "Faerie TokenCreated",
        )?;
    }
    take(
        &|e| {
            e.kind == "AbilityResolved"
                && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                && e.facts.ability.as_deref() == Some(ETB_ABILITY)
        },
        "Hunted Troll ETB resolution",
    )?;
    if matches!(scenario, Scenario::Regeneration) {
        let brainspoil = capture.brainspoil.ok_or("Brainspoil identity missing")?;
        take(
            &|e| {
                e.kind == "SpellManaPaid"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(brainspoil.0)]
                    && e.facts.seats == [cardbench_magic_protocol::SeatId(1)]
            },
            "Brainspoil payment",
        )?;
        take(
            &|e| {
                e.kind == "SpellCast"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(brainspoil.0)]
                    && e.facts.seats == [cardbench_magic_protocol::SeatId(1)]
            },
            "Brainspoil cast",
        )?;
        take(
            &|e| {
                e.kind == "AbilityManaPaid"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                    && e.facts.ability.as_deref() == Some(REGEN_ABILITY)
            },
            "regeneration payment",
        )?;
        take(
            &|e| {
                e.kind == "AbilityActivated"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                    && e.facts.ability.as_deref() == Some(REGEN_ABILITY)
            },
            "regeneration activation",
        )?;
        take(
            &|e| {
                e.kind == "RegenerationShieldCreated"
                    && e.facts.objects
                        == [
                            cardbench_magic_protocol::ObjectRef(troll.0),
                            cardbench_magic_protocol::ObjectRef(troll.0),
                        ]
            },
            "regeneration shield creation",
        )?;
        take(
            &|e| {
                e.kind == "AbilityResolved"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(troll.0)]
                    && e.facts.ability.as_deref() == Some(REGEN_ABILITY)
            },
            "regeneration resolution",
        )?;
        take(
            &|e| {
                e.kind == "RegenerationShieldUsed"
                    && e.facts.objects
                        == [
                            cardbench_magic_protocol::ObjectRef(troll.0),
                            cardbench_magic_protocol::ObjectRef(troll.0),
                        ]
            },
            "regeneration shield replacement",
        )?;
        take(
            &|e| {
                e.kind == "SpellResolved"
                    && e.facts.objects == [cardbench_magic_protocol::ObjectRef(brainspoil.0)]
            },
            "Brainspoil resolution",
        )?;
    }
    take(
        &|e| e.kind == "GameEnded" && e.facts.seats == [cardbench_magic_protocol::SeatId(0)],
        "game end",
    )?;
    Ok(selected)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CANDIDATE_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status{CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn qualification_status() -> &'static str {
    if candidate_kind() == "candidate" {
        "candidate_pilot_pass; independent reviewer approval remains"
    } else {
        "reference_pilot_pass; independent review and candidate trial remain"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).expect("report serializes")
        ),
    )?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n")?;
    Ok(())
}
