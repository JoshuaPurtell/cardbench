//! Full-game RAV Skyknight Legionnaire trace authority for per-card set-PvP work.
//!
//! A legal constructed match casts Skyknight Legionnaire from shuffled sixty-card decks,
//! resolves it, and verifies the same permanent attacks immediately with Flying and Haste. The source
//! file is syntax-gated before its declarative definition is evaluated.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
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
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SKYKNIGHT_LEGIONNAIRE_SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/skyknight_legionnaire.rs";
const DECK_P0: &str = "rav_skyknight_legionnaire_boros_curve";
const DECK_P1: &str = "rav_boros_aggro";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/skyknight_legionnaire.rs"]
mod candidate_skyknight_legionnaire;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Skyknight Legionnaire PvP result: {error}");
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
            eprintln!("Skyknight Legionnaire PvP authority failed: {error}");
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
    let object = game
        .skyknight_legionnaire
        .ok_or_else(|| "Skyknight Legionnaire object missing".to_owned())?;
    if game.events != replay.events
        || game.terminal != replay.terminal
        || game.winner != replay.winner
        || game.skyknight_legionnaire != replay.skyknight_legionnaire
        || project_relevant(&game.events, object) != project_relevant(&replay.events, object)
    {
        return Err("Skyknight Legionnaire full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "power_two_to_three_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Power)?,
        "toughness_two_to_three_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Toughness)?,
        "flying_removed_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Flying)?,
        "haste_removed_mutation_prevents_entry_turn_attack": mutation_reaches_trace_and_fails(seed, Mutation::Haste)?,
        "generic_cost_one_to_two_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Cost)?,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["power_two_to_three_mutation_detected"] != true
        || mutation_sensitivity["toughness_two_to_three_mutation_detected"] != true
        || mutation_sensitivity["flying_removed_mutation_detected"] != true
        || mutation_sensitivity["haste_removed_mutation_prevents_entry_turn_attack"] != true
        || mutation_sensitivity["generic_cost_one_to_two_mutation_detected"] != true
    {
        return Err(format!(
            "a Skyknight Legionnaire definition mutation escaped the trace oracle: {mutation_sensitivity}"
        ));
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
        "card": {"collector_number": 232, "definition_id": "RAV-SKYKNIGHT-LEGIONNAIRE", "name": "Skyknight Legionnaire"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-SKYKNIGHT-LEGIONNAIRE"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: red-white identity and colored payment",
                "117: timing and priority",
                "302.1-302.6: creature permanents and summoning sickness",
                "508: declaring attackers",
                "601.2 and 608.3a: casting and resolving a creature spell",
                "702.9: Flying",
                "702.10: Haste"
            ],
            "source_contract": "RAV printed semantics for the red-white 2/2 Flying/Haste creature",
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
/// that can describe Skyknight Legionnaire. Validation happens before calling the source.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SKYKNIGHT_LEGIONNAIRE_SOURCE_PATH)
        .map_err(|error| format!("could not read Skyknight Legionnaire source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Skyknight Legionnaire source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err(
            "Skyknight Legionnaire source contains an unsupported module attribute".to_owned(),
        );
    }

    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
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
                        "Skyknight Legionnaire source contains an unsupported import".to_owned(),
                    );
                };
                actual_imports.extend(paths);
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => {
                return Err(
                    "Skyknight Legionnaire source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Skyknight Legionnaire imports or item structure differ from the task contract"
                .to_owned(),
        );
    }
    let function =
        definition.ok_or_else(|| "Skyknight Legionnaire definition() is missing".to_owned())?;
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
            "Skyknight Legionnaire function must be a plain definition() -> CardDefinition"
                .to_owned(),
        );
    }
    let Stmt::Expr(Expr::Struct(card), None) = &function.block.stmts[0] else {
        return Err(
            "Skyknight Legionnaire definition must return one direct card literal".to_owned(),
        );
    };
    if !path_is(&card.path, &["CardDefinition"]) || card.rest.is_some() || !card.attrs.is_empty() {
        return Err(
            "Skyknight Legionnaire definition must return a direct CardDefinition literal"
                .to_owned(),
        );
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Skyknight Legionnaire fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err(
                "Skyknight Legionnaire definition contains shorthand or duplicate fields"
                    .to_owned(),
            );
        }
    }
    if fields.len() != 13 {
        return Err(
            "Skyknight Legionnaire definition fields differ from CardDefinition".to_owned(),
        );
    }
    check_string(fields["id"], "RAV-SKYKNIGHT-LEGIONNAIRE")?;
    check_string(fields["name"], "Skyknight Legionnaire")?;
    check_string(fields["set_code"], "RAV")?;
    check_skyknight_mana_cost(fields["mana_cost"])?;
    check_set(fields["colors"], &[&["Color", "Red"], &["Color", "White"]])?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_i16(fields["power"])?;
    check_some_i16(fields["toughness"])?;
    check_skyknight_keywords(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_skyknight_legionnaire::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SKYKNIGHT-LEGIONNAIRE")
        .ok_or_else(|| {
            "RAV-SKYKNIGHT-LEGIONNAIRE is missing from the executable card registry".to_owned()
        })?;
    if submitted.id != runtime.id
        || submitted.name != "Skyknight Legionnaire"
        || submitted.set_code != "RAV"
        || submitted.colors != BTreeSet::from([Color::Red, Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || !submitted.effects.is_empty()
    {
        return Err("RAV-SKYKNIGHT-LEGIONNAIRE identity or card frame differs".to_owned());
    }
    Ok(())
}

/// Rebuild the full RAV rules registry with the validated candidate definition
/// substituted for Skyknight Legionnaire. This makes the cast and state trace exercise the
/// submitted characteristics rather than a hard-coded reference definition.
fn new_rav_game_with_skyknight_legionnaire(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-SKYKNIGHT-LEGIONNAIRE")
        .ok_or_else(|| {
            "RAV-SKYKNIGHT-LEGIONNAIRE is missing from the executable card registry".to_owned()
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
            "Skyknight Legionnaire string field must be {expected:?}"
        ))
    }
}

fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Skyknight Legionnaire boolean field differs".to_owned())
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
        .ok_or_else(|| "Skyknight Legionnaire color set must be a literal array".to_owned())?;
    if items.len() != expected.len()
        || !items
            .iter()
            .zip(expected)
            .all(|(item, path)| check_path(item, path))
    {
        return Err("Skyknight Legionnaire color values differ".to_owned());
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
        .ok_or_else(|| "Skyknight Legionnaire set must use BTreeSet::from".to_owned())?;
    if args.len() != 1 {
        return Err("Skyknight Legionnaire set constructor arity differs".to_owned());
    }
    check_color_array(args[0], expected)
}

fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Skyknight Legionnaire mana colors must be an empty set".to_owned())
    }
}

/// The AST gate permits literal generic-cost mutations while fixing the two
/// printed colored symbols. Payment receipts then detect semantic cost changes.
fn check_skyknight_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"]).ok_or_else(|| {
        "Skyknight Legionnaire mana cost must use ManaCost::with_colors".to_owned()
    })?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= 8)))
    {
        return Err(
            "Skyknight Legionnaire mana cost generic component must be a small literal".to_owned(),
        );
    }
    check_color_array(args[1], &[&["Color", "Red"], &["Color", "White"]])
}

fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err(
            "Skyknight Legionnaire supported rules must be a static string slice".to_owned(),
        );
    };
    if reference.attrs.len() != 0 || reference.mutability.is_some() {
        return Err("Skyknight Legionnaire supported rules reference differs".to_owned());
    }
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Skyknight Legionnaire supported rules must be an array".to_owned())?;
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "flying",
        "haste",
    ];
    if items.len() != expected.len() {
        return Err("Skyknight Legionnaire supported rules differ".to_owned());
    }
    for (item, expected) in items.into_iter().zip(expected) {
        check_string(item, expected)?;
    }
    Ok(())
}

fn check_some_i16(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["Some"]).ok_or_else(|| {
        "Skyknight Legionnaire power and toughness must be fixed literals".to_owned()
    })?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= i16::MAX as u64)))
    {
        Ok(())
    } else {
        Err("Skyknight Legionnaire power or toughness differs".to_owned())
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
        _ => Err("Skyknight Legionnaire keyword and effect lists must be empty".to_owned()),
    }
}

fn check_skyknight_keywords(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Skyknight Legionnaire keywords must use a literal vec!".to_owned());
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return Err("Skyknight Legionnaire keywords must use a literal vec!".to_owned());
    }
    if mac.mac.tokens.is_empty() {
        return Ok(());
    }
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    let items = parser
        .parse2(mac.mac.tokens.clone())
        .map_err(|_| "Skyknight Legionnaire keyword list is not a literal array".to_owned())?;
    let mut seen_flying = false;
    let mut seen_haste = false;
    for item in &items {
        if check_path(item, &["Keyword", "Flying"]) {
            if seen_flying {
                return Err("Skyknight Legionnaire keyword list contains a duplicate".to_owned());
            }
            seen_flying = true;
        } else if check_path(item, &["Keyword", "Haste"]) {
            if seen_haste {
                return Err("Skyknight Legionnaire keyword list contains a duplicate".to_owned());
            }
            seen_haste = true;
        } else {
            return Err("Skyknight Legionnaire has an unsupported keyword".to_owned());
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Mutation {
    Power,
    Toughness,
    Flying,
    Haste,
    Cost,
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut mutated = candidate_skyknight_legionnaire::definition();
    match mutation {
        Mutation::Power => mutated.power = Some(3),
        Mutation::Toughness => mutated.toughness = Some(3),
        Mutation::Flying => mutated
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::Haste => mutated
            .keywords
            .retain(|keyword| *keyword != Keyword::Haste),
        Mutation::Cost => mutated.mana_cost.generic = 2,
    }
    mutated
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let candidate = mutated_definition(mutation);
    let captured = run_match_with(candidate, seed)?;
    let expected_diagnostic = match mutation {
        Mutation::Power => "resolved Skyknight Legionnaire is not a 2/2",
        Mutation::Toughness => "resolved Skyknight Legionnaire is not a 2/2",
        Mutation::Flying => "resolved Skyknight Legionnaire is missing Flying",
        Mutation::Haste => "did not attack on its entry turn",
        Mutation::Cost => "mana payment was",
    };
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains(expected_diagnostic)))
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    skyknight_legionnaire: Option<ObjectId>,
    power_toughness: Option<(i32, i32)>,
    resolved_keywords: Option<Vec<Keyword>>,
    mana_paid: [u8; 3],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_SKYKNIGHT_LEGIONNAIRE_SEED") {
        let seed = seed.parse::<u64>().map_err(|_| {
            "CARDBENCH_SKYKNIGHT_LEGIONNAIRE_SEED must be an unsigned integer".to_owned()
        })?;
        return Ok((seed, run_match(seed)?));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(preview) = run_match_preview(seed)
            && let Some(object) = preview.skyknight_legionnaire
            && preview.mana_paid[0] >= 1
            && preview.mana_paid[1] >= 1
            && preview.mana_paid[2] == 3
        {
            let projected = project_events(&preview.events);
            let cast_turn = projected
                .iter()
                .find(|event| {
                    event.kind == "SpellCast"
                        && event.facts.seats == [cardbench_magic_protocol::SeatId(0)]
                        && event
                            .facts
                            .objects
                            .first()
                            .is_some_and(|card| card.0 == object.0)
                })
                .map(|event| event.turn);
            if cast_turn.is_some_and(|turn| turn <= 5) {
                return Ok((seed, run_match(seed)?));
            }
        }
    }
    Err(format!(
        "no early Skyknight Legionnaire trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with_options(candidate_skyknight_legionnaire::definition(), seed, false)
}

fn run_match_preview(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with_options(candidate_skyknight_legionnaire::definition(), seed, true)
}

fn run_match_with(definition: CardDefinition, seed: u64) -> Result<CapturedMatch, String> {
    run_match_with_options(definition, seed, false)
}

fn run_match_with_options(
    definition: CardDefinition,
    seed: u64,
    stop_after_first_attack: bool,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let deck1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Boros Aggro opponent deck missing")?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-SKYKNIGHT-LEGIONNAIRE".to_owned(),
                count: 4,
            },
        ],
        sideboard: vec![],
    };
    let deck0_count: usize = deck0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    let deck1_count: usize = deck1
        .deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if deck0_count != 60 || deck1_count != 60 {
        return Err(format!(
            "constructed decks have {deck0_count} / {deck1_count} cards; expected 60 each"
        ));
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let policy_cost = definition.mana_cost.clone();
    let mut game = new_rav_game_with_skyknight_legionnaire(definition)?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1.deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SkyknightLegionnairePolicy::new(PlayerId(0), policy_cost)),
        Box::new(PassPolicy),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut preview_complete = false;
    while !game.is_game_over() {
        if stop_after_first_attack && game.turn > 5 {
            break;
        }
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Skyknight Legionnaire game hit a development run bound".to_owned());
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
        let action_debug = format!("{action:?}");
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?} with pool {:?} ({action_debug}): {error}",
                    view.turn, view.step, view.mana_pool,
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if stop_after_first_attack
            && let Some(object) = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-SKYKNIGHT-LEGIONNAIRE") =>
                {
                    Some(*card)
                }
                _ => None,
            })
            && game.event_log.iter().any(|event| {
                matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&object))
            })
        {
            preview_complete = true;
            break;
        }
    }
    let events = game.event_log.clone();
    let skyknight_legionnaire = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == "RAV-SKYKNIGHT-LEGIONNAIRE") =>
        {
            Some(*card)
        }
        _ => None,
    });
    let mut power_toughness = None;
    let mut resolved_keywords = None;
    if let Some(object) = skyknight_legionnaire
        && events
            .iter()
            .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == object))
    {
        let characteristics = game
            .characteristics(object)
            .map_err(|error| error.to_string())?;
        power_toughness = Some((
            characteristics
                .power
                .ok_or("Skyknight Legionnaire has no live power")?,
            characteristics
                .toughness
                .ok_or("Skyknight Legionnaire has no live toughness")?,
        ));
        resolved_keywords = Some(characteristics.keywords.clone());
    }
    let mut mana_paid = [0_u8; 3];
    if let Some(object) = skyknight_legionnaire {
        for event in &events {
            if let GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } = event
                && *card == object
            {
                mana_paid[0] =
                    u8::try_from(colors.iter().filter(|color| **color == Color::Red).count())
                        .unwrap_or(u8::MAX);
                mana_paid[1] = u8::try_from(
                    colors
                        .iter()
                        .filter(|color| **color == Color::White)
                        .count(),
                )
                .unwrap_or(u8::MAX);
                mana_paid[2] = u8::try_from(colors.len()).unwrap_or(u8::MAX);
            }
        }
    }
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: if preview_complete {
            "InProgress".to_owned()
        } else {
            winner.map_or_else(
                || "Draw".to_owned(),
                |seat| format!("Winner(PlayerId({seat}))"),
            )
        },
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        skyknight_legionnaire,
        power_toughness,
        resolved_keywords,
        mana_paid,
    })
}

struct SkyknightLegionnairePolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    cost: ManaCost,
}

struct PassPolicy;

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.skyknight_legionnaire-pass-opponent.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.active_player == view.player && !view.attackers_declared =>
            {
                PolicyAction::DeclareAttackers { attackers: vec![] }
            }
            Step::DeclareBlockers
                if view.active_player != view.player && !view.blockers_declared =>
            {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            }
            _ => PolicyAction::PassPriority,
        }
    }
}

impl SkyknightLegionnairePolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            cost,
        }
    }

    fn play_land(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let has_red = view
            .own_battlefield
            .iter()
            .any(|card| card.mana_colors.contains(&Color::Red));
        let has_white = view
            .own_battlefield
            .iter()
            .any(|card| card.mana_colors.contains(&Color::White));
        let preferred = if !has_red {
            Color::Red
        } else if !has_white {
            Color::White
        } else {
            Color::Red
        };
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

    fn select_generic(&self, view: &GameView) -> Vec<Color> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for colored in &self.cost.colored {
            let Some((_, amount)) = available.iter_mut().find(|(color, _)| color == colored) else {
                return Vec::new();
            };
            *amount = amount.saturating_sub(1);
        }
        let mut selection = Vec::with_capacity(usize::from(self.cost.generic));
        for _ in 0..self.cost.generic {
            let Some((color, amount)) = available.iter_mut().max_by_key(|(_, amount)| *amount)
            else {
                return Vec::new();
            };
            if *amount == 0 {
                return Vec::new();
            }
            *amount -= 1;
            selection.push(*color);
        }
        selection
    }
}

impl CodePolicy for SkyknightLegionnairePolicy {
    fn id(&self) -> &'static str {
        "rav.skyknight_legionnaire-cast-and-attack.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player
            || view.decision_player != self.player
            || view.active_player != self.player
            || view.priority != self.player
        {
            return self.inner.propose_move(view);
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE") && card.can_attack
                })
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.stack_depth != 0 || view.step != Step::PrecombatMain {
            return self.inner.propose_move(view);
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE"))
        {
            let has_red = view.mana_pool.amount(Color::Red) > 0;
            let has_white = view.mana_pool.amount(Color::White) > 0;
            let total = view.mana_pool.total();
            if has_red && has_white && total >= self.cost.mana_value() {
                let generic = self.select_generic(view);
                if generic.len() == usize::from(self.cost.generic) {
                    return PolicyAction::CastWithPayment {
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
                    };
                }
            }
            let needed_color = if !has_red {
                Some(Color::Red)
            } else if !has_white {
                Some(Color::White)
            } else {
                None
            };
            let land = needed_color
                .and_then(|color| {
                    view.own_battlefield.iter().find(|land| {
                        !land.tapped
                            && land.card_types.contains(&CardType::Land)
                            && land.mana_colors.contains(&color)
                    })
                })
                .or_else(|| {
                    view.own_battlefield.iter().find(|land| {
                        !land.tapped
                            && land.card_types.contains(&CardType::Land)
                            && land
                                .mana_colors
                                .iter()
                                .any(|color| matches!(color, Color::Red | Color::White))
                    })
                });
            if let Some(land) = land {
                let color = needed_color
                    .filter(|color| land.mana_colors.contains(color))
                    .or_else(|| {
                        land.mana_colors
                            .iter()
                            .find(|color| matches!(color, Color::Red | Color::White))
                            .copied()
                    })
                    .expect("selected typed land has a Boros color");
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                };
            }
            return PolicyAction::PassPriority;
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
            "Skyknight Legionnaire game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    let object = captured
        .skyknight_legionnaire
        .ok_or("Skyknight Legionnaire was not cast by seat zero")?;
    if captured.power_toughness != Some((2, 2)) {
        return Err(format!(
            "resolved Skyknight Legionnaire is not a 2/2 (observed {:?})",
            captured.power_toughness
        ));
    }
    if captured.mana_paid[0] < 1 || captured.mana_paid[1] < 1 || captured.mana_paid[2] != 3 {
        return Err(format!(
            "Skyknight Legionnaire mana payment was {:?}; expected red 1, white 1, total 3",
            captured.mana_paid
        ));
    }
    let projected = project_events(&captured.events);
    let resolved = projected
        .iter()
        .find(|event| {
            event.kind == "SpellResolved"
                && event
                    .facts
                    .objects
                    .first()
                    .is_some_and(|card| card.0 == object.0)
        })
        .ok_or("Skyknight Legionnaire SpellResolved missing")?;
    let moved = projected
        .iter()
        .find(|event| {
            event.kind == "CardMoved"
                && event
                    .facts
                    .objects
                    .first()
                    .is_some_and(|card| card.0 == object.0)
                && event.facts.zone.as_deref() == Some("Battlefield")
        })
        .ok_or("Skyknight Legionnaire battlefield entry missing")?;
    let cast = projected
        .iter()
        .find(|event| {
            event.kind == "SpellCast"
                && event.facts.seats == [cardbench_magic_protocol::SeatId(0)]
                && event
                    .facts
                    .objects
                    .first()
                    .is_some_and(|card| card.0 == object.0)
        })
        .ok_or("Skyknight Legionnaire SpellCast missing")?;
    if !(cast.sequence < resolved.sequence && resolved.sequence < moved.sequence) {
        return Err(
            "Skyknight Legionnaire cast, resolution, and battlefield-entry order differs"
                .to_owned(),
        );
    }
    let attack = projected
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared"
                && event.facts.seats == [cardbench_magic_protocol::SeatId(0)]
                && event.facts.objects.iter().any(|card| card.0 == object.0)
        })
        .ok_or("Skyknight Legionnaire did not attack in the PvP trace")?;
    if attack.turn != resolved.turn {
        return Err(
            "Skyknight Legionnaire did not attack on its entry turn (Haste interaction failed)"
                .to_owned(),
        );
    }
    let keywords = captured
        .resolved_keywords
        .as_ref()
        .ok_or("Skyknight Legionnaire live keywords missing")?;
    if !keywords.contains(&Keyword::Flying) {
        return Err("resolved Skyknight Legionnaire is missing Flying".to_owned());
    }
    if !keywords.contains(&Keyword::Haste) {
        return Err("resolved Skyknight Legionnaire is missing Haste".to_owned());
    }
    let cast_index = usize::try_from(cast.sequence).map_err(|_| "event index overflow")?;
    let resolved_index = usize::try_from(resolved.sequence).map_err(|_| "event index overflow")?;
    let moved_index = usize::try_from(moved.sequence).map_err(|_| "event index overflow")?;
    let incarnation = captured.events.iter().enumerate().skip(cast_index + 1).find_map(|(index, event)|
        matches!(event, GameEvent::ObjectIncarnationAdvanced { object: card, .. } if *card == object).then_some(index)
    ).ok_or("Skyknight Legionnaire stack incarnation receipt missing")?;
    if !(cast_index < incarnation && incarnation < resolved_index) {
        return Err("Skyknight Legionnaire stack incarnation order differs".to_owned());
    }
    let relevant = project_relevant(&captured.events, object);
    let expected = [
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "GameEnded",
    ];
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    if kinds != expected {
        return Err(format!(
            "Skyknight Legionnaire projected cast/combat event sequence differs: {kinds:?}"
        ));
    }
    let resolved_event = relevant
        .iter()
        .find(|event| event.kind == "SpellResolved")
        .expect("checked above");
    let attack_event = relevant
        .iter()
        .find(|event| event.kind == "AttackersDeclared")
        .expect("checked above");
    if resolved_event.turn != attack_event.turn {
        return Err(
            "Skyknight Legionnaire did not attack on its entry turn (Haste interaction failed)"
                .to_owned(),
        );
    }
    Ok(json!({
        "id": "rav-skyknight-legionnaire-haste-flying-cast-attack",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.skyknight_legionnaire-cast-and-attack.v1", "rav.skyknight_legionnaire-pass-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "skyknight_legionnaire_object": object.0,
        "mana_paid": {"red": captured.mana_paid[0], "white": captured.mana_paid[1], "total": captured.mana_paid[2]},
        "resolved_characteristics": {"power": captured.power_toughness.unwrap().0, "toughness": captured.power_toughness.unwrap().1, "keywords": keywords.iter().map(|keyword| format!("{keyword:?}")).collect::<Vec<_>>()},
        "entry_turn": resolved.turn,
        "attack_turn": attack.turn,
        "ordered_event_sequence": [cast_index, resolved_index, moved_index, usize::try_from(attack.sequence).unwrap()],
        "stack_incarnation_sequence": incarnation,
        "events": relevant,
        "passed": true
    }))
}

fn project_relevant(events: &[GameEvent], object: ObjectId) -> Vec<ProjectedEvent> {
    let mut first_attacker_receipt = false;
    project_events(events)
        .into_iter()
        .filter(|event| {
            if event.kind == "AttackersDeclared"
                && event.facts.objects.iter().any(|card| card.0 == object.0)
            {
                if first_attacker_receipt {
                    false
                } else {
                    first_attacker_receipt = true;
                    true
                }
            } else {
                event.kind == "GameEnded"
                    || matches!(
                        event.kind.as_str(),
                        "CardMoved" | "SpellCast" | "SpellResolved"
                    ) && event
                        .facts
                        .objects
                        .first()
                        .is_some_and(|card| card.0 == object.0)
                        && (event.kind != "CardMoved"
                            || event.facts.zone.as_deref() == Some("Battlefield"))
            }
        })
        .collect()
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
    let passed = definition_id == Some("RAV-SKYKNIGHT-LEGIONNAIRE");
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
