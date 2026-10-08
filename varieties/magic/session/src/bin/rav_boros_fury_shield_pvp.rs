//! Full-game RAV Boros Fury-Shield authority for conditional combat prevention.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BOROS-FURY-SHIELD";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_fury_shield.rs";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/boros_fury_shield.rs"]
mod candidate_boros_fury_shield;

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
            eprintln!("Boros Fury-Shield PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (red_seed, red_game) = find_trace(SpendBranch::Red)?;
    let red_trace = validate_trace(&red_game, red_seed, SpendBranch::Red)?;
    let red_replay = run_match(red_seed, SpendBranch::Red)?;
    if red_game.events != red_replay.events || red_game.winner != red_replay.winner {
        return Err("red-spend Fury-Shield replay was not deterministic".to_owned());
    }
    let (plain_seed, plain_game) = find_trace(SpendBranch::NoRed)?;
    let plain_trace = validate_trace(&plain_game, plain_seed, SpendBranch::NoRed)?;
    let plain_replay = run_match(plain_seed, SpendBranch::NoRed)?;
    if plain_game.events != plain_replay.events || plain_game.winner != plain_replay.winner {
        return Err("no-red Fury-Shield replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "red_conditional_damage_mutation_detected": detects_red_damage_mutation(&red_game),
        "combat_prevention_mutation_detected": detects_prevention_mutation(&red_game),
        "red_payment_receipt_mutation_detected": detects_red_payment_mutation(&red_game),
        "no_red_branch_has_no_controller_damage": !plain_game.events.iter().any(|event| matches!(event,
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. }
                if *source == plain_game.shield
        )),
        "sealed_mutation_authority": false
    });
    for key in [
        "red_conditional_damage_mutation_detected",
        "combat_prevention_mutation_detected",
        "red_payment_receipt_mutation_detected",
        "no_red_branch_has_no_controller_damage",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Boros Fury-Shield mutation escaped the trace oracle: {key}"
            ));
        }
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
        "card": {"collector_number": 5, "definition_id": CARD_ID, "name": "Boros Fury-Shield"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and casting an instant during combat",
                "601.2: target selection, mana payment, and casting",
                "608.2 and 608.3: resolving the prevention effect and its delayed duration",
                "510.1 and 510.2: combat damage assignment and prevention replacement",
                "120.4 and 119.4: red-spend damage and life total changes"
            ],
            "source_contract": "RAV Boros Fury-Shield definition, target creature combat-damage prevention, and red-mana-spent controller-damage branch",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": cardbench_magic_rav::rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [red_trace, plain_trace],
        "per_card_breakdown": breakdown
    }))
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
        return Err(format!(
            "RAV catalog has {} unique names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }
    Ok(catalog)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_boros_fury_shield::definition();
    let expected = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Boros Fury-Shield missing from RAV registry")?;
    if definition.id != CARD_ID
        || definition.name != "Boros Fury-Shield"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != expected.effects
    {
        return Err(
            "Boros Fury-Shield identity, cost, or effect differs from the source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Boros Fury-Shield source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Boros Fury-Shield source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Boros Fury-Shield source has unsupported module items".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Boros Fury-Shield source has an unsupported import".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) if definition.is_none() => definition = Some(function),
            _ => {
                return Err(
                    "Boros Fury-Shield source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports {
        return Err("Boros Fury-Shield imports differ from the task boundary".to_owned());
    }
    let function = definition.ok_or("Boros Fury-Shield definition() is missing")?;
    let literal = direct_struct_function(function, "CardDefinition")?;
    let fields = literal_fields(literal)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Boros Fury-Shield")
        || !check_string(fields["set_code"], "RAV")
        || !check_generic_white_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "White"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Instant"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "prevent-target-creatures-combat-damage-and-red-spend-controller-damage",
            ],
        )
        || !check_none(fields["power"])
        || !check_none(fields["toughness"])
        || !check_empty_vec(fields["keywords"])
        || !check_fury_effect(fields["effects"])
    {
        return Err(
            "Boros Fury-Shield definition is outside its literal source contract".to_owned(),
        );
    }
    Ok(())
}

fn direct_struct_function<'a>(
    function: &'a syn::ItemFn,
    return_type: &str,
) -> Result<&'a syn::ExprStruct, String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    if !attrs_ok
        || !crate_visibility
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || function.block.stmts.len() != 1
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if path_is_type(ty, return_type))
    {
        return Err("Boros Fury-Shield definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Boros Fury-Shield definition() must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Boros Fury-Shield definition() has the wrong literal type".to_owned());
    }
    Ok(value)
}

fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("named fields required".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("duplicate or shorthand field".to_owned());
        }
    }
    Ok(fields)
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, wanted)| {
            segment.ident == *wanted && matches!(&segment.arguments, syn::PathArguments::None)
        })
}
fn path_is_type(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[expected]))
}
fn call_expr<'a>(expr: &'a Expr, owner: &str, method: &str) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Path(function) = &*call.func else {
        return None;
    };
    if function.qself.is_some()
        || !path_is(&function.path, &[owner, method])
        || !call.attrs.is_empty()
    {
        return None;
    }
    Some(call.args.iter().collect())
}
fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
}
fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
}
fn check_none(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["None"]))
}
fn check_empty_set(expr: &Expr) -> bool {
    call_expr(expr, "BTreeSet", "new").is_some_and(|args| args.is_empty())
}
fn check_set(expr: &Expr, expected: &[&[&str]]) -> bool {
    let Some(args) = call_expr(expr, "BTreeSet", "from") else {
        return false;
    };
    if args.len() != 1 {
        return false;
    }
    let Expr::Array(array) = args[0] else {
        return false;
    };
    array.elems.len() == expected.len() && array.elems.iter().zip(expected).all(|(value, path)|
        matches!(value, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, path)))
}
fn check_generic_white_cost(expr: &Expr) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_colors") else {
        return false;
    };
    args.len() == 2
        && matches!(args[0], Expr::Lit(value) if matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u8>().ok() == Some(2)))
        && matches!(args[1], Expr::Array(array) if array.elems.len() == 1
            && matches!(&array.elems[0], Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["Color", "White"])))
}
fn check_rules(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Reference(reference) = expr else {
        return false;
    };
    if !reference.attrs.is_empty() || reference.mutability.is_some() {
        return false;
    }
    let Expr::Array(array) = &*reference.expr else {
        return false;
    };
    array.elems.len() == expected.len()
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(item, text)| check_string(item, text))
}
fn vec_items(expr: &Expr) -> Option<Vec<Expr>> {
    let Expr::Macro(mac) = expr else {
        return None;
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return None;
    }
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(mac.mac.tokens.clone())
        .ok()
        .map(IntoIterator::into_iter)
        .map(Iterator::collect)
}
fn check_empty_vec(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|items| items.is_empty())
}
fn check_fury_effect(expr: &Expr) -> bool {
    let Some(items) = vec_items(expr) else {
        return false;
    };
    if items.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &items[0] else {
        return false;
    };
    if !path_is(
        &effect.path,
        &["Effect", "PreventTargetCreatureCombatDamageUntilEndOfTurn"],
    ) || effect.rest.is_some()
        || effect.fields.len() != 1
    {
        return false;
    }
    let field = &effect.fields[0];
    if !matches!(&field.member, syn::Member::Named(name) if name == "damage_target_controller_equal_to_power_if_mana_color_spent")
    {
        return false;
    }
    matches!(&field.expr, Expr::Call(call) if call.args.len() == 1
        && matches!(&*call.func, Expr::Path(function) if function.qself.is_none() && path_is(&function.path, &["Some"]))
        && matches!(&call.args[0], Expr::Path(color) if color.qself.is_none() && path_is(&color.path, &["Color", "Red"])))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpendBranch {
    Red,
    NoRed,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    shield: ObjectId,
    attacker: ObjectId,
    life: [i64; 2],
}

fn find_trace(branch: SpendBranch) -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BOROS_FURY_SHIELD_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_BOROS_FURY_SHIELD_SEED must be an unsigned integer".to_owned()
        })?;
        let captured = run_match(seed, branch)?;
        validate_trace(&captured, seed, branch)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, branch)
            && validate_trace(&captured, seed, branch).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no terminal Boros Fury-Shield {:?} trace found for seeds 0..={MAX_TRACE_SEED}",
        branch
    ))
}

fn run_match(seed: u64, branch: SpendBranch) -> Result<CapturedMatch, String> {
    let mut game = new_rav_game(candidate_boros_fury_shield::definition())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 17,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 17,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if deck0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || deck1
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Boros Fury-Shield fixture decks must each contain 60 cards".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
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
        Box::new(FuryPolicy::new(PlayerId(0), branch)),
        Box::new(WatchwolfPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Boros Fury-Shield game hit a bounded development limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Fury-Shield policy seat is missing")?;
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let shield = game
        .event_log
        .iter()
        .find_map(|event| match event {
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
        })
        .ok_or("Boros Fury-Shield was not cast from the shuffled library")?;
    let attacker = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::CombatDamagePreventionCreated {
                source, creature, ..
            } if *source == shield => Some(*creature),
            _ => None,
        })
        .ok_or("Boros Fury-Shield did not create source-bound combat prevention")?;
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        shield,
        attacker,
        life: [game.players[0].life, game.players[1].life],
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Boros Fury-Shield missing from RAV definition registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
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

struct FuryPolicy {
    player: PlayerId,
    branch: SpendBranch,
    shield_used: bool,
}
impl FuryPolicy {
    fn new(player: PlayerId, branch: SpendBranch) -> Self {
        Self {
            player,
            branch,
            shield_used: false,
        }
    }
}

impl FuryPolicy {
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let counts = [Color::White, Color::Green, Color::Red].map(|color| {
            view.own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
                })
                .count()
        });
        let preferred = [Color::White, Color::Green, Color::Red]
            .into_iter()
            .zip(counts)
            .min_by_key(|(_, count)| *count)
            .map(|(color, _)| color)?;
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn make_mana(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
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

    fn fury_action(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.shield_used
            || view.step != Step::DeclareAttackers
            || view.active_player == self.player
            || !view.attackers_declared
            || view.stack_depth != 0
            || view.priority != self.player
        {
            return None;
        }
        let target = view
            .combat_attackers
            .iter()
            .find(|attacker| attacker.definition == Some(WATCHWOLF))?;
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let generic_color = match self.branch {
            SpendBranch::Red => Color::Red,
            SpendBranch::NoRed => Color::Green,
        };
        if view.mana_pool.amount(Color::White) == 0 {
            return self.make_mana(view, Color::White);
        }
        if view.mana_pool.amount(generic_color) < 2 {
            return self.make_mana(view, generic_color);
        }
        self.shield_used = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(target.id)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![generic_color, generic_color],
                hybrid: vec![],
            },
        })
    }

    fn watchwolf_action(&self, view: &GameView) -> Option<PolicyAction> {
        if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF))?;
            return Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            }));
        }
        if view.mana_pool.amount(Color::Green) == 0 {
            return self.make_mana(view, Color::Green);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.make_mana(view, Color::White);
        }
        None
    }
}

impl CodePolicy for FuryPolicy {
    fn id(&self) -> &'static str {
        "rav.boros_fury_shield-conditional-combat-prevention.v1"
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
                    .filter(|card| card.definition == Some(WATCHWOLF) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignment = view
                .combat_attackers
                .iter()
                .find(|attacker| attacker.definition == Some(WATCHWOLF))
                .and_then(|attacker| {
                    view.own_battlefield
                        .iter()
                        .find(|blocker| blocker.definition == Some(WATCHWOLF) && blocker.can_block)
                        .map(|blocker| CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        })
                });
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.fury_action(view) {
            return action;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.shield_used {
            return PolicyAction::PassPriority;
        }
        if view
            .hand
            .iter()
            .any(|card| card.definition == Some(WATCHWOLF))
        {
            if let Some(action) = self.watchwolf_action(view) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

struct WatchwolfPolicy {
    player: PlayerId,
}
impl WatchwolfPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl WatchwolfPolicy {
    fn make_mana(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
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
}
impl CodePolicy for WatchwolfPolicy {
    fn id(&self) -> &'static str {
        "rav.watchwolf-pressure-and-combat.v1"
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
                    .filter(|card| card.definition == Some(WATCHWOLF) && card.can_attack)
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
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let counts = [Color::White, Color::Green].map(|color| {
                view.own_battlefield
                    .iter()
                    .filter(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(&color)
                    })
                    .count()
            });
            let preferred = [Color::White, Color::Green]
                .into_iter()
                .zip(counts)
                .min_by_key(|(_, count)| *count)
                .map(|(color, _)| color);
            if let Some(land) = preferred
                .and_then(|color| {
                    view.hand.iter().find(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(&color)
                    })
                })
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF))
        {
            if view.mana_pool.amount(Color::Green) == 0 {
                if let Some(action) = self.make_mana(view, Color::Green) {
                    return action;
                }
            }
            if view.mana_pool.amount(Color::White) == 0 {
                if let Some(action) = self.make_mana(view, Color::White) {
                    return action;
                }
            }
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(
    captured: &CapturedMatch,
    seed: u64,
    branch: SpendBranch,
) -> Result<Value, String> {
    validate_events(captured, &captured.events, seed, branch)
}

fn validate_events(
    captured: &CapturedMatch,
    events: &[GameEvent],
    seed: u64,
    branch: SpendBranch,
) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Boros Fury-Shield full game did not reach a clean seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Boros Fury-Shield match contains rejected policy moves".to_owned());
    }
    let mana = unique_event_index(events, |event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.shield)
    })
    .ok_or("Boros Fury-Shield exact payment receipt is missing or repeated")?;
    let cast = unique_event_index(events, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.shield)
    })
    .ok_or("Boros Fury-Shield cast event is missing or repeated")?;
    let resolved = unique_event_index(events, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == captured.shield)
    })
    .ok_or("Boros Fury-Shield did not resolve exactly once")?;
    let graveyard = unique_event_index(events, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.shield)
    })
    .ok_or("Boros Fury-Shield did not move to its owner's graveyard exactly once")?;
    let attack = events[..mana]
        .iter()
        .rposition(|event| matches!(event,
            GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&captured.attacker)))
        .ok_or("target Watchwolf did not attack before Boros Fury-Shield was cast")?;
    let prevention_created = unique_event_index(events, |event| {
        matches!(event,
        GameEvent::CombatDamagePreventionCreated { source, creature, .. }
            if *source == captured.shield && *creature == captured.attacker)
    })
    .ok_or("Boros Fury-Shield did not create prevention for its actual target")?;
    let prevention_consumed = unique_event_index(events, |event| matches!(event,
        GameEvent::CombatDamagePrevented { source, prevented_by, target: Target::Player(PlayerId(0)), amount: 3 }
            if *source == captured.attacker && *prevented_by == captured.shield))
        .ok_or("target Watchwolf did not have its three combat damage to seat zero prevented")?;
    let step = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > resolved
                && index < prevention_consumed
                && matches!(
                    event,
                    GameEvent::StepBegan {
                        step: Step::CombatDamage,
                        ..
                    }
                ))
            .then_some(index)
        })
        .ok_or("combat-damage step did not resolve after the prevention effect")?;
    let end = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("full shuffled game lacks its terminal seat-zero result")?;

    if !(attack < mana
        && mana < cast
        && cast < prevention_created
        && prevention_created < resolved
        && resolved < graveyard
        && step < prevention_consumed
        && prevention_consumed < end)
    {
        return Err(format!("Boros Fury-Shield event order differs: attack={attack}, mana={mana}, cast={cast}, resolved={resolved}, prevention_created={prevention_created}, graveyard={graveyard}, step={step}, prevented={prevention_consumed}, end={end}; relevant={:?}", events.iter().enumerate().filter(|(_, event)| matches!(event, GameEvent::SpellManaPaid { card, .. } if *card == captured.shield) || matches!(event, GameEvent::SpellCast { card, .. } if *card == captured.shield) || matches!(event, GameEvent::SpellResolved { card } if *card == captured.shield) || matches!(event, GameEvent::CombatDamagePreventionCreated { source, .. } if *source == captured.shield) || matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.shield)).take(30).collect::<Vec<_>>()));
    }
    let payment_colors = match &events[mana] {
        GameEvent::SpellManaPaid { colors, .. } => colors,
        _ => unreachable!(),
    };
    let payment_ok = match branch {
        SpendBranch::Red => {
            payment_colors.len() == 3
                && payment_colors
                    .iter()
                    .filter(|color| **color == Color::Red)
                    .count()
                    == 2
                && payment_colors
                    .iter()
                    .filter(|color| **color == Color::White)
                    .count()
                    == 1
        }
        SpendBranch::NoRed => {
            payment_colors.len() == 3
                && payment_colors
                    .iter()
                    .filter(|color| **color == Color::Green)
                    .count()
                    == 2
                && payment_colors
                    .iter()
                    .filter(|color| **color == Color::White)
                    .count()
                    == 1
                && !payment_colors.contains(&Color::Red)
        }
    };
    if !payment_ok {
        return Err(format!(
            "Boros Fury-Shield {branch:?} trace has wrong exact mana colors: {payment_colors:?}"
        ));
    }

    let source_damage: Vec<_> = events.iter().enumerate().filter(|(_, event)| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == captured.shield)).collect();
    match branch {
        SpendBranch::Red => {
            if source_damage.len() != 1
                || !matches!(
                    source_damage[0].1,
                    GameEvent::DamageDealtToPlayer { amount: 3, .. }
                )
            {
                return Err("red-spend resolution did not deal exactly three damage to the target's controller".to_owned());
            }
            let (index, _) = source_damage[0];
            let before = life_at(events, index.saturating_sub(1));
            let after = life_at(events, index);
            if before[1] - after[1] != 3 || before[0] != after[0] {
                return Err(
                    "red-spend damage did not change the target controller's life by exactly three"
                        .to_owned(),
                );
            }
        }
        SpendBranch::NoRed => {
            if !source_damage.is_empty() {
                return Err(
                    "the no-red payment branch incorrectly damaged the target controller"
                        .to_owned(),
                );
            }
        }
    }
    if events.iter().enumerate().any(|(index, event)| index > prevention_created && index < prevention_consumed && matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), .. } if *source == captured.attacker)) {
        return Err("prevented Watchwolf combat damage still changed seat zero's life".to_owned());
    }
    if let GameEvent::CombatDamagePreventionCreated { expires_turn, .. } =
        events[prevention_created]
    {
        if !matches!(events[attack], GameEvent::AttackersDeclared { .. }) || expires_turn == 0 {
            return Err("prevention effect does not cover the target's declared combat".to_owned());
        }
    }
    let projected = project_events(events);
    let combat_turn = projected
        .get(attack)
        .map(|event| event.turn)
        .ok_or("target attack is outside the projected event stream")?;
    if let GameEvent::CombatDamagePreventionCreated { expires_turn, .. } =
        events[prevention_created]
        && expires_turn != combat_turn
    {
        return Err("prevention did not last through the target's declared combat".to_owned());
    }
    let selected = projected
        .into_iter()
        .filter(|event| relevant_event(event, captured.shield, captured.attacker, combat_turn))
        .collect::<Vec<_>>();
    if selected
        .iter()
        .filter(|event| event.kind == "CombatDamagePrevented")
        .count()
        != 1
    {
        return Err("projected trace must show exactly one prevented combat packet".to_owned());
    }
    let ordered_event_sequence: Vec<_> = selected.iter().map(|event| event.sequence).collect();
    if !ordered_event_sequence
        .windows(2)
        .all(|pair| pair[0] < pair[1])
    {
        return Err("projected event sequence is not strictly increasing".to_owned());
    }
    Ok(json!({
        "id": match branch { SpendBranch::Red => "rav-boros-fury-shield-red-spend", SpendBranch::NoRed => "rav-boros-fury-shield-no-red-spend" },
        "seed": seed,
        "decks": ["custom-rav-boros-fury-shield", "custom-rav-watchwolf-pressure"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "match_end_life": captured.life,
        "boros_fury_shield_object": captured.shield.0,
        "target_watchwolf_object": captured.attacker.0,
        "payment_branch": match branch { SpendBranch::Red => "white-plus-two-red", SpendBranch::NoRed => "white-plus-two-green" },
        "source_damage_to_target_controller": matches!(branch, SpendBranch::Red),
        "combat_damage_prevented_from_target_to_seat_zero": true,
        "ordered_event_sequence": ordered_event_sequence,
        "events": selected,
        "passed": true
    }))
}

fn relevant_event(
    event: &ProjectedEvent,
    shield: ObjectId,
    attacker: ObjectId,
    combat_turn: u32,
) -> bool {
    match event.kind.as_str() {
        "SpellManaPaid" | "SpellCast" | "SpellResolved" => event
            .facts
            .objects
            .first()
            .is_some_and(|object| object.0 == shield.0),
        "CardMoved" => {
            event
                .facts
                .objects
                .first()
                .is_some_and(|object| object.0 == shield.0)
                && event.facts.zone.as_deref() == Some("Graveyard")
        }
        "AttackersDeclared" => {
            event.turn == combat_turn
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == attacker.0)
        }
        "CombatDamagePreventionCreated" => {
            event
                .canonical
                .contains(&format!("source: ObjectId({})", shield.0))
                && event
                    .canonical
                    .contains(&format!("creature: ObjectId({})", attacker.0))
        }
        "CombatDamagePrevented" => {
            event
                .canonical
                .contains(&format!("source: ObjectId({})", attacker.0))
                && event
                    .canonical
                    .contains(&format!("prevented_by: ObjectId({})", shield.0))
        }
        "DamageDealtToPlayer" => event
            .facts
            .objects
            .first()
            .is_some_and(|object| object.0 == shield.0),
        "StepBegan" => {
            event.turn == combat_turn && event.facts.step.as_deref() == Some("CombatDamage")
        }
        "GameEnded" => true,
        _ => false,
    }
}

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index))
        .collect::<Vec<_>>();
    (matches.len() == 1).then(|| matches[0])
}

fn life_at(events: &[GameEvent], end: usize) -> [i64; 2] {
    let mut life = [20_i64; 2];
    for event in events.iter().take(end.saturating_add(1)) {
        match event {
            GameEvent::DamageDealtToPlayer { player, amount, .. } => {
                life[player.0] -= i64::from(*amount)
            }
            GameEvent::LifeLost { player, amount, .. }
            | GameEvent::LifePaid { player, amount, .. } => life[player.0] -= i64::from(*amount),
            GameEvent::AbilityLifePaid { player, amount, .. }
            | GameEvent::ManaAbilityLifePaid { player, amount } => {
                life[player.0] -= i64::from(*amount)
            }
            GameEvent::LandEntryLifePaid { player, amount, .. } => {
                life[player.0] -= i64::from(*amount)
            }
            GameEvent::LifeGained { player, amount } => life[player.0] += i64::from(*amount),
            _ => {}
        }
    }
    life
}

fn detects_red_damage_mutation(captured: &CapturedMatch) -> bool {
    let mut mutant = captured.events.clone();
    let Some(index) = mutant.iter().position(|event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == captured.shield)) else { return false; };
    if let GameEvent::DamageDealtToPlayer { amount, .. } = &mut mutant[index] {
        *amount = 2;
    }
    validate_events(captured, &mutant, 0, SpendBranch::Red).is_err()
}
fn detects_prevention_mutation(captured: &CapturedMatch) -> bool {
    let mut mutant = captured.events.clone();
    let Some(index) = mutant.iter().position(|event| matches!(event,
        GameEvent::CombatDamagePrevented { source, prevented_by, .. } if *source == captured.attacker && *prevented_by == captured.shield)) else { return false; };
    mutant.remove(index);
    validate_events(captured, &mutant, 0, SpendBranch::Red).is_err()
}
fn detects_red_payment_mutation(captured: &CapturedMatch) -> bool {
    let mut mutant = captured.events.clone();
    let Some(event) = mutant.iter_mut().find(|event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.shield)
    }) else {
        return false;
    };
    if let GameEvent::SpellManaPaid { colors, .. } = event {
        let Some(color) = colors.iter_mut().find(|color| **color == Color::Red) else {
            return false;
        };
        *color = Color::Green;
    }
    validate_events(captured, &mutant, 0, SpendBranch::Red).is_err()
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 5;
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited": false
    })
}
