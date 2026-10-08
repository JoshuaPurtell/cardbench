//! Full-game RAV Bottled Cloister linked-hand lifecycle trace authority.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TriggerCondition,
    TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BOTTLED-CLOISTER";
const CARD_NAME: &str = "Bottled Cloister";
const EXILE_ABILITY: &str = "opponent-upkeep-exile-controller-hand-linked";
const RETURN_ABILITY: &str = "controller-upkeep-return-linked-hand-then-draw";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs";
const MAX_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs"]
mod candidate_bottled_cloister;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Bottled Cloister PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Bottled Cloister PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog.iter().any(|card| card.collector_number == 256 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Bottled Cloister identity differs".to_owned());
    }

    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.cloister != replay.cloister
        || captured.watchwolf != replay.watchwolf
        || captured.exiled != replay.exiled
        || captured.returned != replay.returned
        || captured.drawn != replay.drawn
    {
        return Err("Bottled Cloister full-game replay differs".to_owned());
    }

    let mutations = json!({
        "opponent_upkeep_trigger_removal_detected": mutation_detected(seed, Mutation::RemoveExileTrigger)?,
        "controller_upkeep_trigger_removal_detected": mutation_detected(seed, Mutation::RemoveReturnTrigger)?,
        "return_before_draw_order_mutation_detected": mutation_detected(seed, Mutation::DrawBeforeReturn)?,
        "upkeep_draw_removal_detected": mutation_detected(seed, Mutation::RemoveDraw)?,
        "wrong_upkeep_condition_detected": mutation_detected(seed, Mutation::WrongExileCondition)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "opponent_upkeep_trigger_removal_detected",
        "controller_upkeep_trigger_removal_detected",
        "return_before_draw_order_mutation_detected",
        "upkeep_draw_removal_detected",
        "wrong_upkeep_condition_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Bottled Cloister mutation escaped: {key}"));
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
        "card": {"collector_number": 256, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #256 is a colorless {4} artifact. At each opponent's upkeep its controller's current hand is exiled face down and linked to this source incarnation; at its controller's upkeep the still-linked cards return to hand before that player draws one card.",
            "checked_sections": ["400 Zones", "406 Exile", "514 Upkeep step", "603 Triggered abilities", "607 Linked abilities", "608 Resolving spells and abilities", "120 Drawing a card", "601 Casting spells"],
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
        "qualification_status": "unreviewed_diagnostic",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveExileTrigger,
    RemoveReturnTrigger,
    DrawBeforeReturn,
    RemoveDraw,
    WrongExileCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    cloister: Option<ObjectId>,
    cloister_incarnation: Option<u64>,
    watchwolf: Option<ObjectId>,
    cloister_payment: Vec<Color>,
    watchwolf_payment: Vec<Color>,
    first_hand_before_exile: Vec<ObjectId>,
    exiled: Vec<ObjectId>,
    exiled_definitions: Vec<&'static str>,
    exile_source: Option<ObjectId>,
    exile_incarnation: Option<u64>,
    exile_index: Option<usize>,
    exile_hand_empty: bool,
    hidden_from_opponent: bool,
    return_source: Option<ObjectId>,
    return_incarnation: Option<u64>,
    returned: Vec<ObjectId>,
    return_index: Option<usize>,
    drawn: Option<ObjectId>,
    drawn_definition: Option<&'static str>,
    draw_index: Option<usize>,
    draw_library_before: Option<usize>,
    draw_library_after: Option<usize>,
    p0_life: i64,
    p1_life: i64,
}

impl Captured {
    fn new() -> Self {
        Self {
            events: Vec::new(),
            winner: None,
            turns: 0,
            accepted: 0,
            rejected: 0,
            cloister: None,
            cloister_incarnation: None,
            watchwolf: None,
            cloister_payment: Vec::new(),
            watchwolf_payment: Vec::new(),
            first_hand_before_exile: Vec::new(),
            exiled: Vec::new(),
            exiled_definitions: Vec::new(),
            exile_source: None,
            exile_incarnation: None,
            exile_index: None,
            exile_hand_empty: false,
            hidden_from_opponent: false,
            return_source: None,
            return_incarnation: None,
            returned: Vec::new(),
            return_index: None,
            drawn: None,
            drawn_definition: None,
            draw_index: None,
            draw_library_before: None,
            draw_library_after: None,
            p0_life: 0,
            p1_life: 0,
        }
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Bottled Cloister candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Bottled Cloister candidate source is invalid Rust".to_owned())?;
    if file.items.len() != 5 || !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) {
        return Err("Bottled Cloister module must contain only three direct constructors and imports".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(import) if import.attrs.is_empty() => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || function.block.stmts.len() != 1
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.attrs.iter().all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"))
                {
                    return Err(format!("Bottled Cloister {name} is not a direct crate-visible constructor"));
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(format!("Bottled Cloister {name} must return a direct struct literal"));
                };
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "opponent_upkeep_trigger" | "controller_upkeep_trigger" => "TriggeredAbilityBinding",
                    _ => return Err(format!("unexpected Bottled Cloister constructor: {name}")),
                };
                if !literal.path.is_ident(expected) || literal.rest.is_some() {
                    return Err(format!("Bottled Cloister {name} has an unsupported literal shape"));
                }
            }
            _ => return Err("Bottled Cloister module contains executable top-level code".to_owned()),
        }
    }
    if functions != BTreeSet::from([
        "definition".to_owned(),
        "opponent_upkeep_trigger".to_owned(),
        "controller_upkeep_trigger".to_owned(),
    ]) {
        return Err("Bottled Cloister constructor API differs from its task contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let definition = candidate_bottled_cloister::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Bottled Cloister definition missing from the RAV registry")?;
    let opponent = candidate_bottled_cloister::opponent_upkeep_trigger();
    let controller = candidate_bottled_cloister::controller_upkeep_trigger();
    let registered: Vec<_> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if definition != registered_definition || registered.len() != 2
        || !registered.contains(&opponent) || !registered.contains(&controller)
    {
        return Err("Bottled Cloister definition and both triggers must be delegated to its candidate module".to_owned());
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(4)
        || !definition.colors.is_empty()
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Artifact])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules != [
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "opponent-upkeep-linked-hand-exile",
            "controller-upkeep-linked-hand-return-then-draw",
        ]
    {
        return Err("Bottled Cloister identity or mana cost differs from RAV #256".to_owned());
    }
    if opponent.card_definition != CARD_ID
        || opponent.ability.id != EXILE_ABILITY
        || opponent.ability.condition != TriggerCondition::BeginningOfOpponentsUpkeep
        || opponent.ability.mana_cost != ManaCost::new(0)
        || opponent.ability.optional
        || !opponent.ability.targets.is_empty()
        || opponent.ability.effects != [Effect::ExileControllerHandLinkedToSource]
        || controller.card_definition != CARD_ID
        || controller.ability.id != RETURN_ABILITY
        || controller.ability.condition != TriggerCondition::BeginningOfUpkeep
        || controller.ability.mana_cost != ManaCost::new(0)
        || controller.ability.optional
        || !controller.ability.targets.is_empty()
        || controller.ability.effects != [
            Effect::ReturnLinkedHandExileToControllerHand,
            Effect::DrawController,
        ]
    {
        return Err("Bottled Cloister must exile the current hand on opponent upkeep and return it before its own-upkeep draw".to_owned());
    }
    Ok(())
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_BOTTLED_CLOISTER_SEED") {
        let seed = raw.parse::<u64>().map_err(|_| "CARDBENCH_BOTTLED_CLOISTER_SEED must be an unsigned integer".to_owned())?;
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
    Err(format!("no complete Bottled Cloister hand-exile and return trace found for seeds 0..={MAX_SEED}"))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    })
}

fn triggers_with_mutation(mutation: Mutation) -> Vec<TriggeredAbilityBinding> {
    let mut triggers: Vec<_> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .collect();
    let mut exile = candidate_bottled_cloister::opponent_upkeep_trigger();
    let mut returns = candidate_bottled_cloister::controller_upkeep_trigger();
    match mutation {
        Mutation::RemoveExileTrigger => {}
        Mutation::WrongExileCondition => {
            exile.ability.condition = TriggerCondition::BeginningOfUpkeep;
            triggers.push(exile);
        }
        _ => triggers.push(exile),
    }
    match mutation {
        Mutation::RemoveReturnTrigger => {}
        Mutation::DrawBeforeReturn => {
            returns.ability.effects.reverse();
            triggers.push(returns);
        }
        Mutation::RemoveDraw => {
            returns.ability.effects.retain(|effect| *effect != Effect::DrawController);
            triggers.push(returns);
        }
        _ => triggers.push(returns),
    }
    triggers.extend(attachment_triggers());
    triggers
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions.iter_mut().find(|item| item.id == CARD_ID)
        .ok_or("Bottled Cloister missing from registry")? = candidate_bottled_cloister::definition();
    let triggers = triggers_with_mutation(mutation);
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
    ).map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings()).map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings()).map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(rav_static_creature_spell_cost_modifier_bindings()).map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings()).map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings()).map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings()).map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings()).map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings()).map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings()).map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings()).map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(rav_activated_ability_cost_modifier_bindings()).map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(rav_generalized_activated_ability_cost_bindings()).map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings()).map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings()).map_err(|error| error.to_string())?;
    game.validate_invariants().map_err(|error| error.to_string())?;
    Ok(game)
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry { card: CARD_ID.to_owned(), count: 4 },
            DeckEntry { card: WATCHWOLF_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-GLASS-GOLEM".to_owned(), count: 4 },
            DeckEntry { card: "RAV-ISLAND".to_owned(), count: 16 },
            DeckEntry { card: "RAV-FOREST".to_owned(), count: 16 },
            DeckEntry { card: "RAV-PLAINS".to_owned(), count: 16 },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 60 }],
        sideboard: vec![],
    };
    for (name, deck) in [("Bottled Cloister pilot", &pilot), ("pass opponent", &opponent)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards; expected 60"));
        }
    }
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(CloisterPolicy::new(PlayerId(0))),
        Box::new(PassPolicy { player: PlayerId(1) }),
    ];
    let mut capture = Captured::new();
    while !game.is_game_over() {
        if game.turn > 48 || capture.accepted > 8_000 {
            return Err("Bottled Cloister duel exceeded its bounded game horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("Bottled Cloister policy seat missing")?;
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
        let before_hand = game.player(PlayerId(0)).map_err(|error| error.to_string())?.hand.clone();
        let before_library = game.player(PlayerId(0)).map_err(|error| error.to_string())?.library.len();
        let start = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("Bottled Cloister move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        capture.accepted += 1;
        game.validate_invariants().map_err(|error| format!("Bottled Cloister invariant failed: {error}"))?;

        let after = &game.event_log[start..];
        for (offset, event) in after.iter().enumerate() {
            let index = start + offset;
            match event {
                GameEvent::SpellCast { player: PlayerId(0), card } => {
                    if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) {
                        capture.cloister.get_or_insert(*card);
                    } else if game.card_definition(*card).is_ok_and(|definition| definition.id == WATCHWOLF_ID) {
                        capture.watchwolf.get_or_insert(*card);
                    }
                }
                GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } => {
                    if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) {
                        capture.cloister.get_or_insert(*card);
                        capture.cloister_payment = colors.clone();
                    }
                    if game.card_definition(*card).is_ok_and(|definition| definition.id == WATCHWOLF_ID) {
                        capture.watchwolf.get_or_insert(*card);
                        capture.watchwolf_payment = colors.clone();
                    }
                }
                GameEvent::HandExiledWithSource { controller: PlayerId(0), source, source_incarnation, cards }
                    if capture.exile_index.is_none() =>
                {
                    capture.first_hand_before_exile = before_hand.clone();
                    capture.exiled = cards.clone();
                    capture.exiled_definitions = cards.iter().filter_map(|card| game.card_definition(*card).ok().map(|definition| definition.id)).collect();
                    capture.exile_source = Some(*source);
                    capture.exile_incarnation = Some(*source_incarnation);
                    capture.exile_index = Some(index);
                    capture.exile_hand_empty = game.player(PlayerId(0)).map_err(|error| error.to_string())?.hand.is_empty();
                    let opponent_view = game.view_for_player(PlayerId(1)).map_err(|error| error.to_string())?;
                    capture.hidden_from_opponent = cards.iter().all(|card| !view_exposes_card(&opponent_view, *card));
                }
                GameEvent::LinkedHandExileReturned { controller: PlayerId(0), source, source_incarnation, cards }
                    if capture.return_index.is_none() =>
                {
                    capture.return_source = Some(*source);
                    capture.return_incarnation = Some(*source_incarnation);
                    capture.returned = cards.clone();
                    capture.return_index = Some(index);
                    capture.draw_library_before = Some(before_library);
                    capture.draw_library_after = Some(game.player(PlayerId(0)).map_err(|error| error.to_string())?.library.len());
                }
                GameEvent::CardMoved { card, to: Zone::Hand }
                    if capture.return_index.is_some()
                        && capture.drawn.is_none()
                        && !capture.returned.contains(card) =>
                {
                    capture.drawn = Some(*card);
                    capture.drawn_definition = game.card_definition(*card).ok().map(|definition| definition.id);
                    capture.draw_index = Some(index);
                }
                _ => {}
            }
        }
        if capture.cloister.is_some() && capture.cloister_incarnation.is_none() {
            let source = capture.cloister.expect("checked");
            if game.zone_of(source) == Some(Zone::Battlefield) {
                capture.cloister_incarnation = Some(game.object(source).map_err(|error| error.to_string())?.incarnation);
            }
        }
    }

    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.rejected = 0;
    capture.p0_life = game.player(PlayerId(0)).map_err(|error| error.to_string())?.life;
    capture.p1_life = game.player(PlayerId(1)).map_err(|error| error.to_string())?.life;
    Ok(capture)
}

fn view_exposes_card(view: &GameView, object: ObjectId) -> bool {
    view.hand.iter()
        .chain(view.command_zone.iter())
        .chain(view.own_battlefield.iter())
        .chain(view.teammate_battlefield.iter())
        .chain(view.opponent_battlefield.iter())
        .chain(view.combat_attackers.iter())
        .chain(view.stack_spells.iter())
        .any(|card| card.id == object)
}

struct CloisterPolicy {
    player: PlayerId,
    cloister_offered: bool,
    watchwolf_offered: bool,
}
impl CloisterPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player, cloister_offered: false, watchwolf_offered: false }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand.iter()
            .find(|card| card.definition == Some("RAV-FOREST"))
            .or_else(|| view.hand.iter().find(|card| card.definition == Some("RAV-PLAINS")))
            .or_else(|| view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)))
            .map(|land| PolicyAction::PlayLand { card: land.id })
    }

    fn cast(&mut self, view: &GameView, definition: &'static str, cost: ManaCost) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(definition))?;
        if let Some(selection) = select_payment(view, &cost) {
            if definition == CARD_ID {
                self.cloister_offered = true;
            } else {
                self.watchwolf_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            });
        }
        let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
        let desired = missing.or_else(|| {
            (u16::from(view.mana_pool.total())
                < u16::from(cost.generic) + cost.colored.len() as u16)
                .then(|| cost.colored.first().copied().unwrap_or(Color::Blue))
        });
        let land = view.own_battlefield.iter().find(|land| {
            !land.tapped && land.card_types.contains(&CardType::Land)
                && desired.map_or(true, |color| land.mana_colors.contains(&color))
        }).or_else(|| view.own_battlefield.iter().find(|land| {
            !land.tapped && land.card_types.contains(&CardType::Land) && !land.mana_colors.is_empty()
        }))?;
        let color = desired.filter(|color| land.mana_colors.contains(color)
        ).or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }
}

impl CodePolicy for CloisterPolicy {
    fn id(&self) -> &'static str { "rav.bottled-cloister-hand-lifecycle.v1" }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.priority != self.player || view.active_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers {
                attackers: view.own_battlefield.iter().filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack).map(|card| card.id).collect(),
            };
        }
        if view.step == Step::DeclareBlockers && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if !self.cloister_offered
            && let Some(action) = self.cast(view, CARD_ID, ManaCost::new(4))
        {
            return action;
        }
        if self.cloister_offered && !self.watchwolf_offered
            && let Some(action) = self.cast(view, WATCHWOLF_ID, ManaCost::with_colors(0, [Color::Green, Color::White]))
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct PassPolicy { player: PlayerId }
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str { "rav.bottled-cloister-pass-opponent.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        PolicyAction::PassPriority
    }
}

fn select_payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: BTreeMap<Color, u8> = Color::MANA_ALL.into_iter()
        .map(|color| (color, view.mana_pool.amount(color))).collect();
    for color in &cost.colored {
        let amount = available.get_mut(color)?;
        if *amount == 0 { return None; }
        *amount -= 1;
    }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
    for _ in 0..cost.generic {
        let color = available.iter().filter(|(_, amount)| **amount > 0)
            .max_by_key(|(_, amount)| **amount).map(|(color, _)| *color)?;
        let amount = available.get_mut(&color)?;
        *amount -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection { generic, hybrid: vec![] })
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 {
        return Err("Bottled Cloister duel did not end in a clean seat-zero win".to_owned());
    }
    let cloister = captured.cloister.ok_or("pilot did not cast Bottled Cloister")?;
    let cloister_incarnation = captured.cloister_incarnation.ok_or("Cloister battlefield incarnation missing")?;
    let watchwolf = captured.watchwolf.ok_or("pilot did not cast the Watchwolf finisher")?;
    if captured.cloister_payment.len() != 4 {
        return Err(format!("Bottled Cloister must be cast for exactly four generic mana, got {:?}", captured.cloister_payment));
    }
    let exiled: BTreeSet<_> = captured.exiled.iter().copied().collect();
    let before: BTreeSet<_> = captured.first_hand_before_exile.iter().copied().collect();
    let returned: BTreeSet<_> = captured.returned.iter().copied().collect();
    if captured.exiled.len() < 2 || exiled.len() != captured.exiled.len() || before != exiled {
        return Err(format!("opponent-upkeep trigger did not exile the controller's full nonempty hand: before={:?}, exiled={:?}", captured.first_hand_before_exile, captured.exiled));
    }
    if returned != exiled || captured.returned.len() != captured.exiled.len() {
        return Err("controller-upkeep trigger did not return the exact linked hand".to_owned());
    }
    if !captured.exile_hand_empty || !captured.hidden_from_opponent {
        return Err("exiled hand remained in its owner's hand or leaked into the opponent's view".to_owned());
    }
    if captured.exile_source != Some(cloister) || captured.return_source != Some(cloister)
        || captured.exile_incarnation != Some(cloister_incarnation)
        || captured.return_incarnation != Some(cloister_incarnation)
    {
        return Err("linked hand group did not retain the exact Bottled Cloister source incarnation".to_owned());
    }
    let drawn = captured.drawn.ok_or("controller-upkeep trigger did not draw a card after returning the hand")?;
    if exiled.contains(&drawn) || captured.draw_library_before.zip(captured.draw_library_after).is_none_or(|(before, after)| before != after + 1) {
        return Err("Bottled Cloister draw did not move one new library card after the linked return".to_owned());
    }

    let events = &captured.events;
    let cloister_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == cloister && colors.len() == 4)).ok_or("Bottled Cloister exact four-mana payment missing")?;
    let cloister_cast = event_index_after(events, cloister_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == cloister)).ok_or("Bottled Cloister cast event missing")?;
    let cloister_resolved = event_index_after(events, cloister_cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == cloister)).ok_or("Bottled Cloister spell did not resolve")?;
    let cloister_entered = event_index_after(events, cloister_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == cloister)).ok_or("Bottled Cloister did not enter the battlefield")?;

    let exile_stacked = event_index_after(events, cloister_entered, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, source_incarnation, ability: EXILE_ABILITY } if *source == cloister && *source_incarnation == cloister_incarnation)).ok_or("opponent-upkeep linked exile trigger was not stacked from this Cloister incarnation")?;
    let exiled_moves: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        (index > exile_stacked && matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if exiled.contains(card))).then_some(index)
    }).take(captured.exiled.len()).collect();
    if exiled_moves.len() != captured.exiled.len() {
        return Err("not every captured hand card has an Exile movement event".to_owned());
    }
    let hand_exiled = event_index_after(events, *exiled_moves.last().expect("nonempty exiled group"), |event| matches!(event, GameEvent::HandExiledWithSource { controller: PlayerId(0), source, source_incarnation, cards } if *source == cloister && *source_incarnation == cloister_incarnation && cards == &captured.exiled)).ok_or("source-linked hand-exile receipt missing or has wrong incarnation")?;
    let exile_resolved = event_index_after(events, hand_exiled, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: EXILE_ABILITY } if *source == cloister && *source_incarnation == cloister_incarnation)).ok_or("opponent-upkeep exile ability did not resolve")?;

    let return_stacked = event_index_after(events, exile_resolved, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, source_incarnation, ability: RETURN_ABILITY } if *source == cloister && *source_incarnation == cloister_incarnation)).ok_or("controller-upkeep linked return trigger was not stacked from the same Cloister incarnation")?;
    let return_moves: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        (index > return_stacked && matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if exiled.contains(card))).then_some(index)
    }).take(captured.returned.len()).collect();
    if return_moves.len() != captured.returned.len() {
        return Err("not every linked card has a return-to-hand move".to_owned());
    }
    let hand_returned = event_index_after(events, *return_moves.last().expect("nonempty return group"), |event| matches!(event, GameEvent::LinkedHandExileReturned { controller: PlayerId(0), source, source_incarnation, cards } if *source == cloister && *source_incarnation == cloister_incarnation && cards == &captured.returned)).ok_or("linked-hand return receipt missing or has wrong source incarnation")?;
    let return_resolved = event_index_after(events, hand_returned, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: RETURN_ABILITY } if *source == cloister && *source_incarnation == cloister_incarnation)).ok_or("controller-upkeep return-then-draw trigger did not resolve")?;
    let draw_moved = captured.draw_index.ok_or("Bottled Cloister upkeep draw movement missing")?;
    if draw_moved <= hand_returned || draw_moved >= return_resolved || events.get(draw_moved).is_none_or(|event| !matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn)) {
        return Err("Bottled Cloister must draw after returning its linked hand but before the trigger resolves".to_owned());
    }

    let watchwolf_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == watchwolf && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Watchwolf colored payment missing")?;
    let watchwolf_cast = event_index_after(events, watchwolf_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf)).ok_or("Watchwolf cast missing")?;
    let watchwolf_resolved = event_index_after(events, watchwolf_cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf)).ok_or("Watchwolf did not resolve")?;
    let watchwolf_entered = event_index_after(events, watchwolf_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == watchwolf)).ok_or("Watchwolf did not enter the battlefield")?;
    let attack = event_index_after(events, watchwolf_entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("Watchwolf did not attack")?;
    let damage = event_index_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watchwolf)).ok_or("Watchwolf did not deal its expected three combat damage")?;
    let terminal = event_index_after(events, damage, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal PvP win missing after Watchwolf combat")?;

    let mut selected = BTreeSet::from([
        cloister_paid, cloister_cast, cloister_resolved, cloister_entered,
        exile_stacked, hand_exiled, exile_resolved,
        return_stacked, hand_returned, draw_moved, return_resolved,
        watchwolf_paid, watchwolf_cast, watchwolf_resolved, watchwolf_entered, attack, damage, terminal,
    ]);
    selected.extend(exiled_moves);
    selected.extend(return_moves);
    let projected: Vec<ProjectedEvent> = project_events(events).into_iter().enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event)).collect();
    let event_kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    let mut expected = vec!["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "TriggeredAbilityStacked"];
    expected.extend(std::iter::repeat_n("CardMoved", captured.exiled.len()));
    expected.extend(["HandExiledWithSource", "AbilityResolved", "TriggeredAbilityStacked"]);
    expected.extend(std::iter::repeat_n("CardMoved", captured.returned.len()));
    expected.extend(["LinkedHandExileReturned", "CardMoved", "AbilityResolved", "SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved", "AttackersDeclared", "DamageDealtToPlayer", "GameEnded"]);
    if event_kinds != expected {
        return Err(format!("Bottled Cloister ordered lifecycle events differ: {event_kinds:?}"));
    }

    let definitions = &captured.exiled_definitions;
    let drawn_definition = captured.drawn_definition;
    Ok(json!({
        "id": "rav-bottled-cloister-opponent-upkeep-linked-hand-lifecycle",
        "seed": seed,
        "decks": ["rav_bottled_cloister_full_game", "rav_bottled_cloister_pass_opponent"],
        "policies": ["rav.bottled-cloister-hand-lifecycle.v1", "rav.bottled-cloister-pass-opponent.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "mechanics": {
            "cloister_object": cloister.0,
            "cloister_source_incarnation": cloister_incarnation,
            "paid_mana": captured.cloister_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "opponent_upkeep_exile": {
                "ability": EXILE_ABILITY,
                "controller_hand_empty_after_resolution": captured.exile_hand_empty,
                "hidden_from_opponent_view": captured.hidden_from_opponent,
                "source_object": captured.exile_source.map(|object| object.0),
                "source_incarnation": captured.exile_incarnation,
                "exiled_cards": captured.exiled.iter().zip(definitions.iter()).map(|(object, definition)| json!({"object":object.0,"definition_id":definition})).collect::<Vec<_>>(),
                "exiled_count": captured.exiled.len()
            },
            "controller_upkeep_return_then_draw": {
                "ability": RETURN_ABILITY,
                "source_object": captured.return_source.map(|object| object.0),
                "source_incarnation": captured.return_incarnation,
                "returned_objects": captured.returned.iter().map(|object| object.0).collect::<Vec<_>>(),
                "returned_same_linked_group": returned == exiled,
                "drawn_object": drawn.0,
                "drawn_definition_id": drawn_definition,
                "library_size_before_effect_draw": captured.draw_library_before,
                "library_size_after_effect_draw": captured.draw_library_after,
                "draw_after_return_before_ability_resolves": true
            },
            "watchwolf": {
                "object": watchwolf.0,
                "mana_cost": "{G}{W}",
                "payment": captured.watchwolf_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
                "power": 3,
                "toughness": 3,
                "attacked": true,
                "damage": 3
            },
            "source_incarnation_consistent": true
        },
        "exile_trigger_stacked_sequence": exile_stacked,
        "exile_receipt_sequence": hand_exiled,
        "return_trigger_stacked_sequence": return_stacked,
        "linked_return_sequence": hand_returned,
        "draw_sequence": draw_moved,
        "terminal_sequence": terminal,
        "events": projected,
        "passed": true
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
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
}

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
        "qualification": if passed { "reference_pilot_pass; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") { "agent" } else { "reference" }
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
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
