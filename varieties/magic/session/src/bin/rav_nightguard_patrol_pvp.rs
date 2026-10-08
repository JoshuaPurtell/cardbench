//! Full-game RAV Nightguard Patrol trace authority for per-card set-PvP work.
//!
//! A legal constructed match casts Nightguard Patrol from a shuffled sixty-card deck,
//! resolves the creature spell, and checks its live 2/1 First Strike/Vigilance state. The source
//! file is syntax-gated before its declarative definition is evaluated.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, Game, GameEvent, GameView, Keyword, ManaCost,
    ObjectId, PlayerId, PolicyAction, Step, Zone,
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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const NIGHTGUARD_PATROL_SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/nightguard_patrol.rs";
const DECK_P0: &str = "rav_boros_aggro";
const DECK_P1: &str = "rav_golgari_midrange";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/nightguard_patrol.rs"]
mod candidate_nightguard_patrol;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Nightguard Patrol PvP result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Nightguard Patrol PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
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
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        ));
    }

    let (seed, game) = find_trace()?;
    let trace = validate_trace(&game, seed)?;
    let replay = run_match(seed)?;
    if game.events != replay.events
        || game.terminal != replay.terminal
        || game.winner != replay.winner
        || game.nightguard_patrol != replay.nightguard_patrol
        || project_events(&game.events) != project_events(&replay.events)
    {
        return Err("Nightguard Patrol full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "power_two_to_one_mutation_detected_in_game": trace_mutation_detected(Mutation::Power, seed),
        "toughness_one_to_two_mutation_detected_in_game": trace_mutation_detected(Mutation::Toughness, seed),
        "first_strike_removal_detected_in_game": trace_mutation_detected(Mutation::RemoveFirstStrike, seed),
        "vigilance_removal_detected_in_game": trace_mutation_detected(Mutation::RemoveVigilance, seed),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["power_two_to_one_mutation_detected_in_game"] != true
        || mutation_sensitivity["toughness_one_to_two_mutation_detected_in_game"] != true
        || mutation_sensitivity["first_strike_removal_detected_in_game"] != true
        || mutation_sensitivity["vigilance_removal_detected_in_game"] != true
    {
        return Err("a Nightguard Patrol definition mutation escaped the trace oracle".to_owned());
    }

    let breakdown: Vec<Value> = catalog.iter().map(|card| card_status(card)).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 26, "definition_id": "RAV-NIGHTGUARD-PATROL", "name": "Nightguard Patrol"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-NIGHTGUARD-PATROL"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: mana-cost colors and colored payment",
                "117: timing and priority",
                "208.1 and 302.4c: printed power/toughness and creature characteristics",
                "302.1-302.2: casting a creature spell and putting it onto the battlefield",
                "601.2: casting a spell",
                "608.3a: resolving a nontargeted permanent spell",
                "613: continuous effects",
                "508.1a, 508.1f, and 702.20b: declaring attackers, tapping, and vigilance",
                "510.4 and 702.7: first-strike combat damage",
            ],
            "source_contract": "RAV printed semantics for the white 2/1 creature with first strike and vigilance",
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
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

/// Restricts submitted Rust to the exact imports and literal-only card shape
/// that can describe Nightguard Patrol. Validation happens before calling the source.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(NIGHTGUARD_PATROL_SOURCE_PATH)
        .map_err(|error| format!("could not read Nightguard Patrol source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Nightguard Patrol source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Nightguard Patrol source contains an unsupported module attribute".to_owned());
    }

    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "Keyword"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err(
                        "Nightguard Patrol source contains an unsupported import".to_owned()
                    );
                };
                actual_imports.extend(paths);
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => {
                return Err(
                    "Nightguard Patrol source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Nightguard Patrol imports or item structure differ from the task contract".to_owned(),
        );
    }
    let function =
        definition.ok_or_else(|| "Nightguard Patrol definition() is missing".to_owned())?;
    validate_definition_function(function)
}

fn validate_definition_function(function: &syn::ItemFn) -> Result<(), String> {
    let safe_attributes = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    if !safe_attributes
        || !crate_visibility
        || function.sig.ident != "definition"
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.generics.params.len() != 0
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &["CardDefinition"]))
        || function.block.stmts.len() != 1
    {
        return Err(
            "Nightguard Patrol function must be a plain definition() -> CardDefinition".to_owned(),
        );
    }
    let Stmt::Expr(Expr::Struct(card), None) = &function.block.stmts[0] else {
        return Err("Nightguard Patrol definition must return one direct card literal".to_owned());
    };
    if !path_is(&card.path, &["CardDefinition"]) || card.rest.is_some() || !card.attrs.is_empty() {
        return Err(
            "Nightguard Patrol definition must return a direct CardDefinition literal".to_owned(),
        );
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Nightguard Patrol fields must be named".to_owned());
        };
        if !field.attrs.is_empty()
            || field.colon_token.is_none()
            || fields.insert(name.to_string(), &field.expr).is_some()
        {
            return Err(
                "Nightguard Patrol definition contains shorthand or duplicate fields".to_owned(),
            );
        }
    }
    if fields.len() != 13 {
        return Err("Nightguard Patrol definition fields differ from CardDefinition".to_owned());
    }
    check_string(fields["id"], "RAV-NIGHTGUARD-PATROL")?;
    check_string(fields["name"], "Nightguard Patrol")?;
    check_string(fields["set_code"], "RAV")?;
    check_mana_cost(fields["mana_cost"])?;
    check_set(fields["colors"], &[&["Color", "White"]])?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_i16(fields["power"])?;
    check_some_i16(fields["toughness"])?;
    check_allowed_keywords(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_nightguard_patrol::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NIGHTGUARD-PATROL")
        .ok_or_else(|| {
            "RAV-NIGHTGUARD-PATROL is missing from the executable card registry".to_owned()
        })?;
    if submitted.id != runtime.id
        || submitted.name != "Nightguard Patrol"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(2, [Color::White])
        || submitted.colors != BTreeSet::from([Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || !submitted.effects.is_empty()
    {
        return Err("RAV-NIGHTGUARD-PATROL identity or printed characteristics differ".to_owned());
    }
    Ok(())
}

/// Rebuild the full RAV rules registry with the validated candidate definition
/// substituted for Nightguard Patrol. This makes the cast and state trace exercise the
/// submitted characteristics rather than a hard-coded reference definition.
fn new_rav_game_with_nightguard_patrol(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-NIGHTGUARD-PATROL")
        .ok_or_else(|| {
            "RAV-NIGHTGUARD-PATROL is missing from the executable card registry".to_owned()
        })?;
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

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
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
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}

fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}

fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(&segment.arguments, PathArguments::None)
            })
}

fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
    {
        Ok(())
    } else {
        Err(format!(
            "Nightguard Patrol string field must be {expected:?}"
        ))
    }
}

fn check_int(expr: &Expr, expected: u64) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
}

fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Nightguard Patrol boolean field differs".to_owned())
    }
}

fn check_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() && path_is(&path.path, expected))
}

fn array_items<'a>(expr: &'a Expr) -> Option<Vec<&'a Expr>> {
    match expr {
        Expr::Array(array) if array.attrs.is_empty() => Some(array.elems.iter().collect()),
        _ => None,
    }
}

fn check_color_array(expr: &Expr, expected: &[&[&str]]) -> Result<(), String> {
    let items = array_items(expr)
        .ok_or_else(|| "Nightguard Patrol color set must be a literal array".to_owned())?;
    if items.len() != expected.len()
        || !items
            .iter()
            .zip(expected)
            .all(|(item, path)| check_path(item, path))
    {
        return Err("Nightguard Patrol color values differ".to_owned());
    }
    Ok(())
}

fn call_expr<'a>(expr: &'a Expr, path: &[&str]) -> Option<Vec<&'a Expr>> {
    match expr {
        Expr::Call(call) if call.attrs.is_empty() && check_path(&call.func, path) => {
            Some(call.args.iter().collect())
        }
        _ => None,
    }
}

fn check_set(expr: &Expr, expected: &[&[&str]]) -> Result<(), String> {
    let args = call_expr(expr, &["BTreeSet", "from"])
        .ok_or_else(|| "Nightguard Patrol set must use BTreeSet::from".to_owned())?;
    if args.len() != 1 {
        return Err("Nightguard Patrol set constructor arity differs".to_owned());
    }
    check_color_array(args[0], expected)
}

fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Nightguard Patrol mana colors must be an empty set".to_owned())
    }
}

fn check_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"])
        .ok_or_else(|| "Nightguard Patrol mana cost must use ManaCost::with_colors".to_owned())?;
    if args.len() != 2 || !check_int(args[0], 2) {
        return Err("Nightguard Patrol mana cost must have two generic mana".to_owned());
    }
    check_color_array(args[1], &[&["Color", "White"]])
}

fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Nightguard Patrol supported rules must be a static string slice".to_owned());
    };
    if reference.attrs.len() != 0 || reference.mutability.is_some() {
        return Err("Nightguard Patrol supported rules reference differs".to_owned());
    }
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Nightguard Patrol supported rules must be an array".to_owned())?;
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "first-strike",
        "vigilance",
    ];
    if items.len() != expected.len() {
        return Err("Nightguard Patrol supported rules differ".to_owned());
    }
    for (item, expected) in items.into_iter().zip(expected) {
        check_string(item, expected)?;
    }
    Ok(())
}

fn check_some_i16(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["Some"])
        .ok_or_else(|| "Nightguard Patrol power and toughness must be fixed literals".to_owned())?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= i16::MAX as u64)))
    {
        Ok(())
    } else {
        Err("Nightguard Patrol power or toughness differs".to_owned())
    }
}

fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    match expr {
        Expr::Macro(mac)
            if mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && mac.mac.tokens.is_empty() =>
        {
            Ok(())
        }
        _ => Err("Nightguard Patrol effect list must be empty".to_owned()),
    }
}

/// Permit a missing printed keyword so the live trace, rather than a source
/// preflight check, rejects the semantic mutant. No other expressions pass.
fn check_allowed_keywords(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Nightguard Patrol keywords must use a literal vec!".to_owned());
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return Err("Nightguard Patrol keywords must use a literal vec!".to_owned());
    }
    if mac.mac.tokens.is_empty() {
        return Err("Nightguard Patrol keywords must include a printed keyword".to_owned());
    }
    let mut seen = BTreeSet::new();
    for token in mac.mac.tokens.to_string().split(',').map(str::trim) {
        let keyword = match token {
            "Keyword :: FirstStrike" => "first-strike",
            "Keyword :: Vigilance" => "vigilance",
            _ => {
                return Err(
                    "Nightguard Patrol keyword list contains an unsupported expression".to_owned(),
                );
            }
        };
        if !seen.insert(keyword) {
            return Err("Nightguard Patrol keyword list contains a duplicate".to_owned());
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Mutation {
    Power,
    Toughness,
    RemoveFirstStrike,
    RemoveVigilance,
}

fn trace_mutation_detected(mutation: Mutation, seed: u64) -> bool {
    let mut mutated = candidate_nightguard_patrol::definition();
    match mutation {
        Mutation::Power => mutated.power = Some(1),
        Mutation::Toughness => mutated.toughness = Some(2),
        Mutation::RemoveFirstStrike => mutated
            .keywords
            .retain(|keyword| *keyword != Keyword::FirstStrike),
        Mutation::RemoveVigilance => mutated
            .keywords
            .retain(|keyword| *keyword != Keyword::Vigilance),
    }
    let expected = match mutation {
        Mutation::Power => "resolved Nightguard Patrol is not a 2/1",
        Mutation::Toughness => "resolved Nightguard Patrol is not a 2/1",
        Mutation::RemoveFirstStrike => "resolved Nightguard Patrol is missing FirstStrike",
        Mutation::RemoveVigilance => "Nightguard Patrol tapped when it attacked without Vigilance",
    };
    matches!(
        run_match_with_definition(seed, mutated)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| ())),
        Err(error) if error.contains(expected)
    )
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    nightguard_patrol: Option<ObjectId>,
    power_toughness: Option<(i32, i32)>,
    resolved_keywords: Option<Vec<Keyword>>,
    mana_available_at_cast: [u8; 2],
    mana_paid: [u8; 2],
    vigilance_attack_untapped: Option<bool>,
    vigilance_attack_live: Option<bool>,
    combat_blocker: Option<ObjectId>,
    blocker_graveyard_after_combat: Option<bool>,
    patrol_tapped_after_combat: Option<bool>,
    patrol_damage_after_combat: Option<i32>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_NIGHTGUARD_PATROL_SEED") {
        let seed = seed.parse::<u64>().map_err(|_| {
            "CARDBENCH_NIGHTGUARD_PATROL_SEED must be an unsigned integer".to_owned()
        })?;
        return Ok((seed, run_match(seed)?));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed) {
            if captured.terminal == "Winner(PlayerId(0))"
                && captured.winner == Some(0)
                && captured.rejected_moves == 0
                && captured.nightguard_patrol.is_some()
                && captured.mana_available_at_cast[0] > 0
                && captured.mana_available_at_cast[1] >= 3
                && captured.mana_paid[0] > 0
                && captured.mana_paid[1] >= 3
                && validate_trace(&captured, seed).is_ok()
            {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no clean Nightguard Patrol trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with_definition(seed, candidate_nightguard_patrol::definition())
}

fn run_match_with_definition(
    seed: u64,
    definition: CardDefinition,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let deck0 = decks
        .iter()
        .find(|deck| deck.id == DECK_P0)
        .ok_or("Boros Aggro deck missing")?;
    let deck1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Selesnya Midrange deck missing")?;
    for deck in [deck0, deck1] {
        let count: usize = deck
            .deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{} has {count} cards; expected 60", deck.id));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_rav_game_with_nightguard_patrol(definition)?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0.deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1.deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(NightguardPatrolPolicy::new(PlayerId(0))),
        Box::new(GolgariBlockPolicy::new(PlayerId(1))),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut resolved_characteristics = None;
    let mut resolved_keywords = None;
    let mut mana_available_at_cast = [0_u8; 2];
    let mut vigilance_attack_untapped = None;
    let mut vigilance_attack_live = None;
    let mut combat_blocker = None;
    let mut blocker_graveyard_after_combat = None;
    let mut patrol_tapped_after_combat = None;
    let mut patrol_damage_after_combat = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Nightguard Patrol game hit a development run bound".to_owned());
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
        let cast_request = match &action {
            PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } => {
                Some(request)
            }
            _ => None,
        };
        if let Some(request) = cast_request
            && view.hand.iter().any(|card| {
                card.id == request.card && card.definition == Some("RAV-NIGHTGUARD-PATROL")
            })
        {
            mana_available_at_cast = [view.mana_pool.amount(Color::White), view.mana_pool.total()];
        }
        let previous_event_count = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("policy move rejected: {error}"))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let new_events = &game.event_log[previous_event_count..];
        if vigilance_attack_untapped.is_none()
            && let Some(patrol) = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-NIGHTGUARD-PATROL") =>
                {
                    Some(*card)
                }
                _ => None,
            })
            && new_events.iter().any(|event| {
                matches!(event,
                GameEvent::AttackersDeclared { player: PlayerId(0), attackers }
                    if attackers.contains(&patrol))
            })
        {
            let view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            let patrol_view = view
                .own_battlefield
                .iter()
                .find(|card| card.id == patrol)
                .ok_or(
                    "attacking Nightguard Patrol is missing from its controller's battlefield",
                )?;
            vigilance_attack_untapped = Some(!patrol_view.tapped);
            vigilance_attack_live = Some(
                patrol_view.keywords.contains(&Keyword::Vigilance)
                    && view.combat_attackers.iter().any(|card| card.id == patrol),
            );
        }
        if combat_blocker.is_none()
            && let Some(patrol) = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-NIGHTGUARD-PATROL") =>
                {
                    Some(*card)
                }
                _ => None,
            })
            && let Some(blocker) = new_events.iter().find_map(|event| match event {
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    assignments,
                } => assignments.iter().find_map(|(attacker, blocker)| {
                    (*attacker == patrol
                        && game
                            .card_definition(*blocker)
                            .is_ok_and(|definition| definition.id == "RAV-ELVES-OF-DEEP-SHADOW"))
                    .then_some(*blocker)
                }),
                _ => None,
            })
        {
            combat_blocker = Some(blocker);
        }
        if let (Some(patrol), Some(blocker)) = (
            game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-NIGHTGUARD-PATROL") =>
                {
                    Some(*card)
                }
                _ => None,
            }),
            combat_blocker,
        ) && blocker_graveyard_after_combat.is_none()
            && new_events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::StepBegan {
                        active_player: PlayerId(0),
                        step: Step::CombatDamage,
                        ..
                    }
                )
            })
            && game.event_log.iter().any(|event| {
                matches!(event,
                GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
                    if *source == patrol && *permanent == blocker)
            })
            && game.event_log.iter().any(|event| {
                matches!(event,
                GameEvent::StateBasedAction { card, reason: "creature has lethal damage" }
                    if *card == blocker)
            })
        {
            blocker_graveyard_after_combat = Some(game.zone_of(blocker) == Some(Zone::Graveyard));
            let patrol_object = game.object(patrol).map_err(|error| error.to_string())?;
            patrol_tapped_after_combat = Some(patrol_object.tapped);
            patrol_damage_after_combat = Some(patrol_object.damage);
        }
        if resolved_characteristics.is_none() {
            if let Some(object) = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-NIGHTGUARD-PATROL") =>
                {
                    Some(*card)
                }
                _ => None,
            }) && game
                .event_log
                .iter()
                .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == object))
            {
                let characteristics = game
                    .characteristics(object)
                    .map_err(|error| error.to_string())?;
                resolved_characteristics = Some((
                    characteristics
                        .power
                        .ok_or("Nightguard Patrol has no live power")?,
                    characteristics
                        .toughness
                        .ok_or("Nightguard Patrol has no live toughness")?,
                ));
                resolved_keywords = Some(characteristics.keywords);
            }
        }
    }
    let events = game.event_log.clone();
    let nightguard_patrol = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == "RAV-NIGHTGUARD-PATROL") =>
        {
            Some(*card)
        }
        _ => None,
    });
    if nightguard_patrol.is_none() {
        return Err(format!(
            "Nightguard Patrol was not cast; terminal={}, winner={:?}, turns={}, accepted_moves={accepted}",
            if game.winner().is_some() {
                format!("Winner(PlayerId({}))", game.winner().unwrap().0)
            } else {
                "Draw".to_owned()
            },
            game.winner().map(|player| player.0),
            game.turn,
        ));
    }
    let nightguard_patrol_object = nightguard_patrol.expect("checked above");
    let mut mana_paid = [0_u8; 2];
    for event in &events {
        if let GameEvent::SpellManaPaid {
            player: PlayerId(0),
            card,
            colors,
        } = event
            && *card == nightguard_patrol_object
        {
            for color in colors {
                match color {
                    Color::White => {
                        mana_paid[0] = mana_paid[0].saturating_add(1);
                        mana_paid[1] = mana_paid[1].saturating_add(1);
                    }
                    _ => mana_paid[1] = mana_paid[1].saturating_add(1),
                }
            }
        }
    }
    let power_toughness = resolved_characteristics;
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: if winner.is_some() {
            format!("Winner(PlayerId({}))", winner.unwrap())
        } else {
            "Draw".to_owned()
        },
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        nightguard_patrol,
        power_toughness,
        resolved_keywords,
        mana_available_at_cast,
        mana_paid,
        vigilance_attack_untapped,
        vigilance_attack_live,
        combat_blocker,
        blocker_graveyard_after_combat,
        patrol_tapped_after_combat,
        patrol_damage_after_combat,
    })
}

struct NightguardPatrolPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
}

struct GolgariBlockPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
}

impl GolgariBlockPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}

impl CodePolicy for GolgariBlockPolicy {
    fn id(&self) -> &'static str {
        "rav.nightguard_patrol-golgari-block.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.active_player == self.player
                    && view.player == self.player
                    && !view.attackers_declared =>
            {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            Step::DeclareBlockers
                if view.active_player != self.player
                    && view.player == self.player
                    && !view.blockers_declared =>
            {
                return PolicyAction::DeclareBlockers {
                    assignments: view
                        .combat_attackers
                        .iter()
                        .find(|card| card.definition == Some("RAV-NIGHTGUARD-PATROL"))
                        .and_then(|attacker| {
                            view.own_battlefield
                                .iter()
                                .find(|card| {
                                    card.definition == Some("RAV-ELVES-OF-DEEP-SHADOW")
                                        && card.can_block
                                        && !card.tapped
                                })
                                .map(|blocker| CombatBlock {
                                    attacker: attacker.id,
                                    blocker: blocker.id,
                                })
                        })
                        .into_iter()
                        .collect(),
                };
            }
            _ => {}
        }
        if view.player == self.player
            && view.decision_player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0 {
                let action = self.inner.propose_move(view);
                if matches!(
                    action,
                    PolicyAction::PlayLand { .. }
                        | PolicyAction::PlayLandWithEntryLifePayment { .. }
                ) {
                    return action;
                }
            }
            let elf_in_play_or_stack = view
                .own_battlefield
                .iter()
                .chain(view.stack_spells.iter())
                .any(|card| card.definition == Some("RAV-ELVES-OF-DEEP-SHADOW"));
            if !elf_in_play_or_stack
                && let Some(elf) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-ELVES-OF-DEEP-SHADOW"))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                        card: elf.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
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
            }
        }
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
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

impl NightguardPatrolPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}

impl CodePolicy for NightguardPatrolPolicy {
    fn id(&self) -> &'static str {
        "rav.nightguard_patrol-cast.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.decision_player == self.player
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && view
                    .hand
                    .iter()
                    .any(|card| card.card_types.contains(&CardType::Land))
            {
                let action = self.inner.propose_move(view);
                if matches!(
                    action,
                    PolicyAction::PlayLand { .. }
                        | PolicyAction::PlayLandWithEntryLifePayment { .. }
                ) {
                    return action;
                }
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-NIGHTGUARD-PATROL"))
            {
                let white = view.mana_pool.amount(Color::White) >= 1;
                let mut generic = Vec::new();
                let mut remaining = 2_u8;
                for color in [
                    Color::Green,
                    Color::Blue,
                    Color::Black,
                    Color::Red,
                    Color::White,
                ] {
                    let reserve_colored_symbol = u8::from(color == Color::White);
                    let available = view
                        .mana_pool
                        .amount(color)
                        .saturating_sub(reserve_colored_symbol);
                    for _ in 0..available.min(remaining) {
                        generic.push(color);
                        remaining -= 1;
                    }
                }
                if white && view.mana_pool.total() >= 3 && remaining == 0 {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: cardbench_magic_engine::ManaPaymentSelection {
                            generic,
                            hybrid: vec![],
                        },
                    };
                }
                let needed = if !white {
                    Some(Color::White)
                } else if view.mana_pool.total() < 3 {
                    view.own_battlefield
                        .iter()
                        .find(|land| {
                            !land.tapped
                                && land.card_types.contains(&CardType::Land)
                                && !land.mana_colors.is_empty()
                        })
                        .and_then(|land| land.mana_colors.iter().next().copied())
                } else {
                    None
                };
                if let Some(color) = needed
                    && let Some(land) = view.own_battlefield.iter().find(|land| {
                        !land.tapped
                            && land.card_types.contains(&CardType::Land)
                            && land.mana_colors.contains(&color)
                    })
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color,
                    };
                }
            }
        }
        self.inner.propose_move(view)
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(
            "Nightguard Patrol game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    let object = captured
        .nightguard_patrol
        .ok_or("Nightguard Patrol was not cast by seat zero")?;
    if captured.power_toughness != Some((2, 1)) {
        return Err(format!(
            "resolved Nightguard Patrol is not a 2/1 (observed {:?})",
            captured.power_toughness
        ));
    }
    if captured.mana_available_at_cast[0] == 0 || captured.mana_available_at_cast[1] < 3 {
        return Err("Nightguard Patrol cast lacked one white and two generic mana".to_owned());
    }
    if captured.mana_paid[0] == 0 || captured.mana_paid[1] != 3 {
        return Err("Nightguard Patrol did not spend two generic mana plus white".to_owned());
    }
    let keywords = captured
        .resolved_keywords
        .as_ref()
        .ok_or("Nightguard Patrol live keywords missing")?;
    if !keywords.contains(&Keyword::FirstStrike) {
        return Err("resolved Nightguard Patrol is missing FirstStrike".to_owned());
    }
    if captured.vigilance_attack_untapped != Some(true) {
        return Err("Nightguard Patrol tapped when it attacked without Vigilance".to_owned());
    }
    if captured.vigilance_attack_live != Some(true) {
        return Err("Nightguard Patrol attack did not retain live Vigilance".to_owned());
    }
    if !keywords.contains(&Keyword::Vigilance) {
        return Err("resolved Nightguard Patrol is missing Vigilance".to_owned());
    }
    let blocker = captured
        .combat_blocker
        .ok_or("Nightguard Patrol was not blocked by Elves of Deep Shadow")?;
    if captured.blocker_graveyard_after_combat != Some(true)
        || captured.patrol_tapped_after_combat != Some(false)
        || captured.patrol_damage_after_combat != Some(0)
    {
        return Err(
            "post-combat state does not show First Strike killing the 1/1 blocker before damage"
                .to_owned(),
        );
    }
    let paid = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == object))
        .ok_or("Nightguard Patrol SpellManaPaid event missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == object)).ok_or("Nightguard Patrol SpellCast missing")?;
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == object))
        .ok_or("Nightguard Patrol SpellResolved missing")?;
    let battlefield = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == object)).ok_or("Nightguard Patrol battlefield move missing")?;
    let attackers = captured
        .events
        .iter()
        .enumerate()
        .skip(battlefield + 1)
        .find_map(|(index, event)| {
            matches!(event,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers }
                if attackers.contains(&object))
            .then_some(index)
        })
        .ok_or("Nightguard Patrol AttackersDeclared event missing")?;
    let blockers = captured
        .events
        .iter()
        .enumerate()
        .skip(attackers + 1)
        .find_map(|(index, event)| {
            matches!(event,
            GameEvent::BlockersDeclared { player: PlayerId(1), assignments }
                if assignments.contains(&(object, blocker)))
            .then_some(index)
        })
        .ok_or("Nightguard Patrol block assignment missing")?;
    let first_strike_step = captured
        .events
        .iter()
        .enumerate()
        .skip(blockers + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    active_player: PlayerId(0),
                    step: Step::FirstStrikeCombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("FirstStrikeCombatDamage step receipt missing")?;
    let first_strike_damage = captured
        .events
        .iter()
        .enumerate()
        .skip(first_strike_step + 1)
        .find_map(|(index, event)| {
            matches!(event,
            GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
                if *source == object && *permanent == blocker)
            .then_some(index)
        })
        .ok_or("Nightguard Patrol did not deal two First Strike damage to its blocker")?;
    let blocker_death = captured
        .events
        .iter()
        .enumerate()
        .skip(first_strike_damage + 1)
        .find_map(|(index, event)| {
            matches!(event,
            GameEvent::StateBasedAction { card, reason: "creature has lethal damage" }
                if *card == blocker)
            .then_some(index)
        })
        .ok_or("lethal damage state-based action for the 1/1 blocker missing")?;
    let blocker_to_graveyard = captured
        .events
        .iter()
        .enumerate()
        .skip(blocker_death + 1)
        .find_map(|(index, event)| {
            matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard }
                if *card == blocker)
            .then_some(index)
        })
        .ok_or("First Strike blocker was not moved to its graveyard")?;
    let combat_damage_step = captured
        .events
        .iter()
        .enumerate()
        .skip(blocker_to_graveyard + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    active_player: PlayerId(0),
                    step: Step::CombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("ordinary CombatDamage step receipt missing")?;
    if captured.events.iter().any(|event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount }
            if *source == blocker && *permanent == object && *amount > 0)
    }) {
        return Err(
            "the 1/1 blocker dealt damage to Nightguard Patrol after dying to First Strike"
                .to_owned(),
        );
    }
    let cast_incarnation = captured
        .events
        .iter()
        .enumerate()
        .skip(cast + 1)
        .find_map(|(index, event)| matches!(event, GameEvent::ObjectIncarnationAdvanced { object: card, .. } if *card == object).then_some(index))
        .ok_or("Nightguard Patrol stack incarnation receipt missing")?;
    if !(paid < cast
        && cast < cast_incarnation
        && cast_incarnation < resolved
        && resolved < battlefield)
    {
        return Err("Nightguard Patrol cast, resolution, and entry order differs".to_owned());
    }
    let terminal = captured
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("GameEnded win receipt missing")?;
    let selected = [
        paid,
        cast,
        resolved,
        battlefield,
        attackers,
        blockers,
        first_strike_step,
        first_strike_damage,
        blocker_death,
        blocker_to_graveyard,
        combat_damage_step,
        terminal,
    ];
    if selected.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("cast and combat receipts are not in chronological order".to_owned());
    }
    let projected = project_events(&captured.events);
    let relevant: Vec<_> = selected
        .iter()
        .map(|index| projected[*index].clone())
        .collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "StepBegan",
        "DamageDealtToPermanent",
        "StateBasedAction",
        "CardMoved",
        "StepBegan",
        "GameEnded",
    ];
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    if kinds != expected {
        return Err(format!(
            "Nightguard Patrol projected event sequence differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-nightguard-patrol-colored-creature-cast",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.nightguard_patrol-cast.v1", "rav.nightguard_patrol-golgari-block.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "nightguard_patrol_object": object.0,
        "mana_paid": {"white": captured.mana_paid[0], "total": captured.mana_paid[1]},
        "mana_available_at_cast": {"white": captured.mana_available_at_cast[0], "total": captured.mana_available_at_cast[1]},
        "resolved_characteristics": {"power": captured.power_toughness.unwrap().0, "toughness": captured.power_toughness.unwrap().1, "keywords": keywords.iter().map(|keyword| format!("{keyword:?}")).collect::<Vec<_>>()},
        "combat": {
            "attacker_object": object.0,
            "attacker_untapped_after_declaration": captured.vigilance_attack_untapped,
            "attacker_had_live_vigilance_and_was_attacking": captured.vigilance_attack_live,
            "blocker_object": blocker.0,
            "blocker_definition": "RAV-ELVES-OF-DEEP-SHADOW",
            "blocker_in_graveyard_after_combat": captured.blocker_graveyard_after_combat,
            "attacker_tapped_after_combat": captured.patrol_tapped_after_combat,
            "attacker_damage_after_combat": captured.patrol_damage_after_combat,
            "first_strike_damage_event_index": first_strike_damage,
            "blocker_death_event_index": blocker_death,
            "normal_damage_step_event_index": combat_damage_step
        },
        "ordered_event_sequence": [paid, cast, resolved, battlefield],
        "stack_incarnation_sequence": cast_incarnation,
        "events": relevant,
        "passed": true
    }))
}

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut data = String::new();
    for event in events {
        let _ = writeln!(data, "{event:?}");
    }
    let hash = data
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some("RAV-NIGHTGUARD-PATROL");
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
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
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
