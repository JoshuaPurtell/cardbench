//! Full-game diagnostic authority for RAV Conclave Phalanx.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ConvokeContribution, ConvokePayment, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CONCLAVE-PHALANX";
const CARD_NAME: &str = "Conclave Phalanx";
const TRIGGER_ID: &str = "etb-gain-life-per-controlled-creature";
const ELVES_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SCATTER_ID: &str = "RAV-SCATTER-THE-SEEDS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclave_phalanx.rs";
const MAX_TRACE_SEED: u64 = 192;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/conclave_phalanx.rs"]
mod candidate_conclave_phalanx;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Conclave Phalanx PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    validate_candidate_trigger()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay)
        || project_events(&captured.events) != project_events(&replay.events)
    {
        return Err("Conclave Phalanx shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "convoke_generic_cost_increased_mutation_detected_in_game": mutation_detected(seed, Mutation::MoreGeneric),
        "global_instead_of_controlled_creature_count_detected_in_game": mutation_detected(seed, Mutation::CountAllCreatures),
        "sealed_mutation_authority": false
    });
    if mutations["convoke_removed_mutation_detected_in_game"] != true
        || mutations["convoke_generic_cost_increased_mutation_detected_in_game"] != true
        || mutations["global_instead_of_controlled_creature_count_detected_in_game"] != true
    {
        return Err(
            "a Conclave Phalanx compiled gameplay mutant escaped the trace oracle".to_owned(),
        );
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 10, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: white mana cost and payment",
                "117: timing and priority",
                "302.1-302.2 and 601.2: creature spell casting and resolution",
                "603.2 and 603.6a: enters-the-battlefield trigger",
                "608.2: triggered ability resolution",
                "702.50: Convoke generic payment",
                "119: life totals and life gain"
            ],
            "source_contract": "RAV Conclave Phalanx is a white 2/4 Creature with {4}{W}, Convoke, and an ETB trigger that gains life equal to the number of creatures its controller controls as the trigger resolves",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
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
        return Err("RAV catalog identity or 291/306 denominator changed".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Conclave Phalanx source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Conclave Phalanx source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Conclave Phalanx module has unsupported attributes".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut imports)?;
            }
            Item::Fn(function) => {
                if functions
                    .insert(function.sig.ident.to_string(), function)
                    .is_some()
                {
                    return Err("Conclave Phalanx has duplicate candidate functions".to_owned());
                }
                check_literal_function(function)?;
            }
            _ => return Err("Conclave Phalanx source contains executable module items".to_owned()),
        }
    }
    if imports != expected_imports
        || functions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from(["definition", "triggered_ability"])
        || file.items.len() != 4
    {
        return Err(
            "Conclave Phalanx source imports or module shape differ from the contract".to_owned(),
        );
    }
    Ok(())
}

fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, output)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            output.insert(next);
            Ok(())
        }
        UseTree::Group(group) => {
            for child in &group.items {
                flatten_use(prefix, child, output)?;
            }
            Ok(())
        }
        UseTree::Rename(_) | UseTree::Glob(_) => {
            Err("Conclave Phalanx imports may not rename or glob".to_owned())
        }
    }
}

fn check_literal_function(function: &syn::ItemFn) -> Result<(), String> {
    let name = function.sig.ident.to_string();
    let result = match name.as_str() {
        "definition" => "CardDefinition",
        "triggered_ability" => "TriggeredAbilityBinding",
        _ => return Err("Conclave Phalanx contains an unexpected function".to_owned()),
    };
    let attrs_ok = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if !attrs_ok
        || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
        || function.sig.inputs.len() != 0
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.path.is_ident(result)))
        || function.block.stmts.len() != 1
    {
        return Err(format!(
            "Conclave Phalanx {name} must be an item-local literal function"
        ));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Conclave Phalanx {name} must return a direct struct literal"
        ));
    };
    if value.rest.is_some()
        || value
            .fields
            .iter()
            .any(|field| field.colon_token.is_none() || !safe_expr(&field.expr))
    {
        return Err(format!(
            "Conclave Phalanx {name} contains an unsafe or shorthand field"
        ));
    }
    Ok(())
}

fn safe_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            value.attrs.is_empty()
                && value.qself.is_none()
                && [
                    "Color::White",
                    "CardType::Creature",
                    "Keyword::Convoke",
                    "TriggerCondition::EntersBattlefield",
                    "Effect::GainLifeForEachControlledCreature",
                ]
                .iter()
                .any(|path| path_string(&value.path) == *path)
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                "BTreeSet::from",
                "BTreeSet::new",
                "ManaCost::with_colors",
                "ManaCost::new",
                "Some",
            ]
            .iter()
            .any(|expected| path_string(&path.path) == *expected)
        }
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.rest.is_none()
                && [
                    "CardDefinition",
                    "TriggeredAbilityBinding",
                    "TriggeredAbility",
                ]
                .iter()
                .any(|expected| path_string(&value.path) == *expected)
                && value.fields.iter().all(|field| {
                    field.attrs.is_empty() && field.colon_token.is_some() && safe_expr(&field.expr)
                })
        }
        Expr::Macro(value) => {
            value.attrs.is_empty()
                && value.mac.path.is_ident("vec")
                && (value.mac.tokens.is_empty()
                    || value.mac.tokens.to_string() == "Keyword :: Convoke"
                    || value.mac.tokens.to_string()
                        == "Effect :: GainLifeForEachControlledCreature")
        }
        _ => false,
    }
}

fn path_string(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_conclave_phalanx::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Conclave Phalanx is missing from the RAV registry")?;
    if definition != runtime
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(4, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(4)
        || definition.keywords != [Keyword::Convoke]
        || !definition.effects.is_empty()
    {
        return Err(
            "Conclave Phalanx identity, {4}{W} cost, 2/4 body, or Convoke differs".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_trigger() -> Result<(), String> {
    let trigger = candidate_conclave_phalanx::triggered_ability();
    let runtime = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Conclave Phalanx ETB trigger is missing from the RAV registry")?;
    if trigger != runtime
        || trigger.card_definition != CARD_ID
        || trigger.ability.condition != cardbench_magic_engine::TriggerCondition::EntersBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::GainLifeForEachControlledCreature]
    {
        return Err(
            "Conclave Phalanx ETB life-count trigger differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    MoreGeneric,
    CountAllCreatures,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    phalanx: Option<ObjectId>,
    scatter: Option<ObjectId>,
    opponent_elf: Option<ObjectId>,
    convokers: Vec<ObjectId>,
    convoker_tapped_at_cast: Vec<bool>,
    payment_colors: Vec<Color>,
    token_objects: Vec<ObjectId>,
    trigger_stacked: bool,
    trigger_resolved: bool,
    life_gain: Option<i32>,
    life_after: Option<i64>,
    controlled_creatures_at_gain: Option<usize>,
    opponent_creatures_at_gain: Option<usize>,
    stats_at_entry: Option<(i32, i32, bool)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CONCLAVE_PHALANX_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_CONCLAVE_PHALANX_SEED must be an unsigned integer".to_owned()
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
        "no complete Conclave Phalanx Convoke/ETB trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_conclave_phalanx::definition();
    let mut trigger = candidate_conclave_phalanx::triggered_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::MoreGeneric => definition.mana_cost = ManaCost::with_colors(5, [Color::White]),
        Mutation::CountAllCreatures => {
            trigger.ability.effects = vec![Effect::GainLifeForEachCreature]
        }
    }
    let mut game = new_rav_game(definition, trigger)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ELVES_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SCATTER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 44,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ELVES_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
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
        return Err(
            "Conclave Phalanx fixtures must be legal 60-card singleton-name-limited decks"
                .to_owned(),
        );
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 60,
        max_policy_moves: 12_000,
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
        Box::new(PhalanxPolicy::new(PlayerId(0))),
        Box::new(OpponentPolicy::new(PlayerId(1))),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        phalanx: None,
        scatter: None,
        opponent_elf: None,
        convokers: vec![],
        convoker_tapped_at_cast: vec![],
        payment_colors: vec![],
        token_objects: vec![],
        trigger_stacked: false,
        trigger_resolved: false,
        life_gain: None,
        life_after: None,
        controlled_creatures_at_gain: None,
        opponent_creatures_at_gain: None,
        stats_at_entry: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Conclave Phalanx full game hit its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Phalanx policy seat missing")?;
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
                    "Conclave Phalanx policy action failed on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    captured.rejected_moves = 0;
    Ok(captured)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut CapturedMatch) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.phalanx = Some(*card)
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == SCATTER_ID) =>
            {
                captured.scatter = Some(*card)
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == ELVES_ID) =>
            {
                captured.opponent_elf = Some(*card)
            }
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } => {
                captured.convokers.push(*creature);
                captured.convoker_tapped_at_cast.push(
                    game.object(*creature)
                        .map_err(|error| error.to_string())?
                        .tapped,
                );
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.payment_colors = colors.clone()
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => captured.token_objects.push(*token),
            GameEvent::TriggeredAbilityStacked {
                controller: PlayerId(0),
                source,
                ability: TRIGGER_ID,
                ..
            } if Some(*source) == captured.phalanx => captured.trigger_stacked = true,
            GameEvent::AbilityResolved {
                source,
                ability: TRIGGER_ID,
                ..
            } if Some(*source) == captured.phalanx => captured.trigger_resolved = true,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount,
            } if captured.life_gain.is_none() && captured.phalanx.is_some() => {
                captured.life_gain = Some(*amount);
                let own = game
                    .view_for_player(PlayerId(0))
                    .map_err(|error| error.to_string())?;
                let opposing = game
                    .view_for_player(PlayerId(1))
                    .map_err(|error| error.to_string())?;
                captured.life_after = Some(own.own_life);
                captured.controlled_creatures_at_gain = Some(
                    own.own_battlefield
                        .iter()
                        .filter(|card| card.card_types.contains(&CardType::Creature))
                        .count(),
                );
                captured.opponent_creatures_at_gain = Some(
                    opposing
                        .own_battlefield
                        .iter()
                        .filter(|card| card.card_types.contains(&CardType::Creature))
                        .count(),
                );
            }
            _ => {}
        }
    }
    if let Some(phalanx) = captured.phalanx
        && captured.stats_at_entry.is_none()
        && game.zone_of(phalanx) == Some(Zone::Battlefield)
    {
        let stats = game
            .characteristics(phalanx)
            .map_err(|error| error.to_string())?;
        captured.stats_at_entry = Some((
            stats.power.ok_or("Phalanx has no live power")?,
            stats.toughness.ok_or("Phalanx has no live toughness")?,
            stats.keywords.contains(&Keyword::Convoke),
        ));
    }
    Ok(())
}

fn new_rav_game(
    definition: CardDefinition,
    trigger: TriggeredAbilityBinding,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Conclave Phalanx definition registry entry missing")?;
    *registered = definition.clone();
    let mut triggers = rav_triggered_ability_bindings();
    let registered_trigger = triggers
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Conclave Phalanx trigger registry entry missing")?;
    *registered_trigger = trigger;
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

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let slot = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
        if slot.1 == 0 {
            return None;
        }
        slot.1 -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let index = available
            .iter()
            .position(|(color, count)| *count > 0 && *color == Color::Green)
            .or_else(|| available.iter().position(|(_, count)| *count > 0))?;
        let slot = &mut available[index];
        slot.1 -= 1;
        generic.push(slot.0);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

struct PhalanxPolicy {
    player: PlayerId,
}
impl PhalanxPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color,
            })
    }
    fn land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let forests = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-FOREST"))
            .count();
        let plains = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-PLAINS"))
            .count();
        let desired = if plains == 0 && forests >= 5 {
            "RAV-PLAINS"
        } else {
            "RAV-FOREST"
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(desired))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
    fn cast_elf(view: &GameView) -> Option<PolicyAction> {
        if view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(ELVES_ID))
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(ELVES_ID))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return Self::mana_source(view, Color::Green);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
    fn cast_scatter(view: &GameView) -> Option<PolicyAction> {
        let creature_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Creature))
            .count();
        if creature_count != 1
            || view
                .hand
                .iter()
                .all(|card| card.definition != Some(CARD_ID))
        {
            return None;
        }
        let lands = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land))
            .count();
        if lands < 5 {
            return None;
        }
        let spell = view
            .hand
            .iter()
            .find(|card| card.definition == Some(SCATTER_ID))?;
        let Some(mana_selection) = payment_selection(
            view,
            &ManaCost::with_colors(3, [Color::Green, Color::Green]),
        ) else {
            if let Some(color) = [Color::Green, Color::Green]
                .into_iter()
                .find(|color| view.mana_pool.amount(*color) == 0)
            {
                return Self::mana_source(view, color);
            }
            return Color::MANA_ALL
                .into_iter()
                .find_map(|color| Self::mana_source(view, color));
        };
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: spell.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection,
        })
    }
    fn cast_phalanx(view: &GameView) -> Option<PolicyAction> {
        if view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
            || view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(CARD_ID))
        {
            return None;
        }
        let opponent_elf = view
            .opponent_battlefield
            .iter()
            .any(|card| card.definition == Some(ELVES_ID));
        let contributors: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Creature) && !card.tapped)
            .take(4)
            .collect();
        if contributors.len() != 4 || !opponent_elf {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::White) == 0 {
            return Self::mana_source(view, Color::White);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: contributors
                    .iter()
                    .map(|creature| ConvokePayment {
                        creature: creature.id,
                        contribution: ConvokeContribution::Generic,
                    })
                    .collect(),
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
}
impl CodePolicy for PhalanxPolicy {
    fn id(&self) -> &'static str {
        "rav.conclave-phalanx-convoke-etb-life.v1"
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
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = Self::land(view) {
            return action;
        }
        if let Some(action) = Self::cast_elf(view) {
            return action;
        }
        if let Some(action) = Self::cast_scatter(view) {
            return action;
        }
        if let Some(action) = Self::cast_phalanx(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct OpponentPolicy {
    player: PlayerId,
    elf_cast: bool,
}
impl OpponentPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_cast: false,
        }
    }
}
impl CodePolicy for OpponentPolicy {
    fn id(&self) -> &'static str {
        "rav.phalanx-opponent-one-creature.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
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
        if view.decision_player != self.player
            || view.priority != self.player
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
                .find(|card| card.definition == Some("RAV-FOREST"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.elf_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ELVES_ID))
        {
            if view.mana_pool.amount(Color::Green) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && land.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            } else {
                self.elf_cast = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Conclave Phalanx full shuffled game did not finish with a P0 win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Conclave Phalanx reference game has rejected scripted moves".to_owned());
    }
    let phalanx = captured.phalanx.ok_or("Conclave Phalanx was never cast")?;
    let scatter = captured
        .scatter
        .ok_or("Scatter the Seeds fixture did not create convoke bodies")?;
    let opponent_elf = captured
        .opponent_elf
        .ok_or("opponent creature was not present for controlled-count discrimination")?;
    if captured.convokers.len() != 4
        || captured.convokers.iter().collect::<BTreeSet<_>>().len() != 4
        || captured.convoker_tapped_at_cast != [true; 4]
    {
        return Err(format!(
            "Conclave Phalanx did not tap four distinct controlled creatures for Convoke: contributors={:?}, tapped={:?}, phalanx={:?}, scatter={:?}, tokens={:?}, events={:?}",
            captured.convokers,
            captured.convoker_tapped_at_cast,
            captured.phalanx,
            captured.scatter,
            captured.token_objects,
            captured
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::ConvokeUsed { .. }
                        | GameEvent::SpellCast { .. }
                        | GameEvent::GameEnded { .. }
                ))
                .collect::<Vec<_>>()
        ));
    }
    if captured.payment_colors != [Color::White] {
        return Err(format!(
            "Conclave Phalanx did not pay exactly one white mana for its {{4}}{{W}} cost: {:?}",
            captured.payment_colors
        ));
    }
    if captured.token_objects.len() != 3 {
        return Err("Scatter the Seeds did not create exactly three fixture creatures".to_owned());
    }
    if captured.stats_at_entry != Some((2, 4, true)) {
        return Err("live entering Conclave Phalanx is not a 2/4 with Convoke".to_owned());
    }
    if !captured.trigger_stacked || !captured.trigger_resolved {
        return Err("Conclave Phalanx ETB trigger did not stack and resolve".to_owned());
    }
    if captured.life_gain != Some(5)
        || captured.life_after != Some(25)
        || captured.controlled_creatures_at_gain != Some(5)
        || captured.opponent_creatures_at_gain != Some(1)
    {
        return Err(format!(
            "Phalanx life count is not the five creatures its controller controls (gain={:?}, life={:?}, controlled={:?}, opposing={:?})",
            captured.life_gain,
            captured.life_after,
            captured.controlled_creatures_at_gain,
            captured.opponent_creatures_at_gain
        ));
    }
    let events = &captured.events;
    let convoke: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature)).then_some(index)).collect();
    if convoke.len() != 4 {
        return Err("typed trace lacks exactly four generic Convoke receipts".to_owned());
    }
    let payment = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == phalanx && colors.as_slice() == [Color::White])).ok_or("exact white SpellManaPaid receipt is missing")?;
    let cast = unique_event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == phalanx)).ok_or("Phalanx SpellCast receipt is missing")?;
    let resolution = unique_event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == phalanx),
    )
    .ok_or("Phalanx SpellResolved receipt is missing")?;
    let entry = unique_event_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == phalanx)).ok_or("Phalanx battlefield entry receipt is missing")?;
    let trigger = unique_event_index(events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == phalanx)).ok_or("Phalanx ETB trigger stack receipt is missing")?;
    let life = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount: 5
            }
        )
    })
    .ok_or("Phalanx five-life gain receipt is missing")?;
    let ability = unique_event_index(events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == phalanx)).ok_or("Phalanx ETB AbilityResolved receipt is missing")?;
    let scatter_cast = unique_event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == scatter)).ok_or("Scatter the Seeds cast receipt is missing")?;
    let scatter_resolution = unique_event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == scatter),
    )
    .ok_or("Scatter the Seeds resolution receipt is missing")?;
    let opponent_cast = unique_event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == opponent_elf)).ok_or("opponent creature spell cast receipt is missing")?;
    let opponent_entry = unique_event_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == opponent_elf)).ok_or("opponent creature battlefield entry receipt is missing")?;
    let attack = first_event_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&phalanx))).ok_or("Phalanx did not attack in the full game")?;
    let blockers = first_event_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("opponent did not make the legal empty blocker declaration after Phalanx attacked")?;
    let damage = first_event_index(events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == phalanx)).ok_or("Phalanx dealt no combat damage to the opponent")?;
    let end = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal game winner receipt is missing")?;
    if convoke.iter().any(|index| *index >= payment)
        || payment >= cast
        || cast >= resolution
        || entry >= trigger
        || resolution >= trigger
        || trigger >= life
        || life >= ability
        || scatter_cast >= scatter_resolution
        || scatter_resolution >= convoke[0]
        || opponent_cast >= opponent_entry
        || opponent_entry >= trigger
        || ability >= attack
        || attack >= blockers
        || blockers >= damage
        || damage >= end
    {
        return Err(
            "Conclave Phalanx cast, ETB life trigger, combat, and end events are out of order"
                .to_owned(),
        );
    }
    let evidence_indices: BTreeSet<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        let support_card = match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card }
            | GameEvent::CardMoved { card, .. } => {
                *card == scatter || *card == opponent_elf || captured.convokers.contains(card)
            }
            _ => false,
        };
        let include = support_card || matches!(event,
            GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature)
        ) || matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } | GameEvent::SpellCast { player: PlayerId(0), card } | GameEvent::SpellResolved { card } if *card == phalanx
        ) || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == phalanx)
            || matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == phalanx)
            || matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == phalanx)
            || matches!(event, GameEvent::LifeGained { player: PlayerId(0), amount: 5 })
            || matches!(event, GameEvent::TokenCreated { player: PlayerId(0), .. })
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&phalanx))
            || index == blockers
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == phalanx)
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        include.then_some(index)
    }).collect();
    let projected_all = project_events(events);
    let mut projected = Vec::new();
    for index in &evidence_indices {
        let mut value = serde_json::to_value(
            projected_all
                .get(*index)
                .ok_or("projected event was missing")?,
        )
        .map_err(|error| error.to_string())?;
        if let GameEvent::ConvokeUsed {
            player,
            creature,
            contribution: None,
        } = &events[*index]
        {
            value["facts"] =
                json!({"seats": [player.0], "objects": [creature.0], "contribution": "Generic"});
        }
        projected.push(value);
    }
    let kinds: Vec<_> = projected
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    if kinds.iter().filter(|kind| **kind == "ConvokeUsed").count() != 4
        || !kinds.contains(&"LifeGained")
    {
        return Err("projected Phalanx witness stream lost Convoke or life-gain events".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-conclave-phalanx-convoke-etb-life",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "rejected_policy_moves": captured.rejected_moves,
        "full_game_event_count": events.len(),
        "mechanics": {
            "source_spell": phalanx.0,
            "fixture_spell": scatter.0,
            "opponent_creature": opponent_elf.0,
            "mana_cost": "{4}{W}",
            "generic_paid_by_convoke": 4,
            "convoke_contributors": captured.convokers.iter().map(|object| object.0).collect::<Vec<_>>(),
            "convoke_contribution": "Generic",
            "convoke_creatures_tapped": captured.convoker_tapped_at_cast,
            "colored_mana_paid": captured.payment_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "live_characteristics": {"power": 2, "toughness": 4, "convoke": captured.stats_at_entry.map(|stats| stats.2)},
            "fixture_tokens_created": captured.token_objects.iter().map(|object| object.0).collect::<Vec<_>>(),
            "trigger_stacked": captured.trigger_stacked,
            "trigger_resolved": captured.trigger_resolved,
            "controlled_creatures_at_life_gain": captured.controlled_creatures_at_gain,
            "opponent_creatures_at_life_gain": captured.opponent_creatures_at_gain,
            "life_gained": captured.life_gain,
            "controller_life_after_trigger": captured.life_after,
            "attacked_and_dealt_combat_damage": true
        },
        "events": projected
    }))
}

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matching = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matching.next()?;
    matching.next().is_none().then_some(first)
}
fn first_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(predicate)
}
fn first_event_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}
fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.phalanx == right.phalanx
        && left.scatter == right.scatter
        && left.opponent_elf == right.opponent_elf
        && left.convokers == right.convokers
        && left.convoker_tapped_at_cast == right.convoker_tapped_at_cast
        && left.payment_colors == right.payment_colors
        && left.token_objects == right.token_objects
        && left.trigger_stacked == right.trigger_stacked
        && left.trigger_resolved == right.trigger_resolved
        && left.life_gain == right.life_gain
        && left.life_after == right.life_after
        && left.controlled_creatures_at_gain == right.controlled_creatures_at_gain
        && left.opponent_creatures_at_gain == right.opponent_creatures_at_gain
        && left.stats_at_entry == right.stats_at_entry
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 10;
    json!({
        "collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "diagnostic_only" } else { "not_evaluated" },
        "credited": false
    })
}
