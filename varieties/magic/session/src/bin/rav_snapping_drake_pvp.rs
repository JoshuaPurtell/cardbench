//! Full-game RAV Snapping Drake trace authority for per-card set-PvP work.
//!
//! A legal constructed match casts Snapping Drake from a shuffled sixty-card deck,
//! resolves the creature spell, and checks the live 3/2 Flying permanent. The source
//! file is syntax-gated before its declarative definition is evaluated.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
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
const SNAPPING_DRAKE_SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/snapping_drake.rs";
const DECK_P0: &str = "rav_snapping_drake_mono_blue";
const DECK_P1: &str = "rav_boros_aggro";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/snapping_drake.rs"]
mod candidate_snapping_drake;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Snapping Drake PvP result: {error}");
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
            eprintln!("Snapping Drake PvP authority failed: {error}");
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
        .snapping_drake
        .ok_or_else(|| "Snapping Drake object missing".to_owned())?;
    if game.events != replay.events
        || game.terminal != replay.terminal
        || game.winner != replay.winner
        || game.snapping_drake != replay.snapping_drake
        || project_relevant(&game.events, object) != project_relevant(&replay.events, object)
    {
        return Err("Snapping Drake full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "power_three_to_two_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Power)?,
        "toughness_two_to_three_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Toughness)?,
        "flying_removed_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Flying)?,
        "generic_cost_three_to_four_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::Cost)?,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["power_three_to_two_mutation_detected"] != true
        || mutation_sensitivity["toughness_two_to_three_mutation_detected"] != true
        || mutation_sensitivity["flying_removed_mutation_detected"] != true
        || mutation_sensitivity["generic_cost_three_to_four_mutation_detected"] != true
    {
        return Err("a Snapping Drake definition mutation escaped the trace oracle".to_owned());
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
        "card": {"collector_number": 64, "definition_id": "RAV-SNAPPING-DRAKE", "name": "Snapping Drake"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-SNAPPING-DRAKE"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Timing and Priority",
                "302 Creatures",
                "508 Declaring Attackers",
                "601 Casting Spells",
                "608 Resolving Spells and Abilities",
                "702.9 Flying"
            ],
            "source_contract": "RAV printed semantics for the blue Flying creature",
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
/// that can describe Snapping Drake. Validation happens before calling the source.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SNAPPING_DRAKE_SOURCE_PATH)
        .map_err(|error| format!("could not read Snapping Drake source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Snapping Drake source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Snapping Drake source contains an unsupported module attribute".to_owned());
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
                    return Err("Snapping Drake source contains an unsupported import".to_owned());
                };
                actual_imports.extend(paths);
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => return Err("Snapping Drake source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Snapping Drake imports or item structure differ from the task contract".to_owned(),
        );
    }
    let function = definition.ok_or_else(|| "Snapping Drake definition() is missing".to_owned())?;
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
            "Snapping Drake function must be a plain definition() -> CardDefinition".to_owned(),
        );
    }
    let Stmt::Expr(Expr::Struct(card), None) = &function.block.stmts[0] else {
        return Err("Snapping Drake definition must return one direct card literal".to_owned());
    };
    if !path_is(&card.path, &["CardDefinition"]) || card.rest.is_some() || !card.attrs.is_empty() {
        return Err(
            "Snapping Drake definition must return a direct CardDefinition literal".to_owned(),
        );
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Snapping Drake fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err(
                "Snapping Drake definition contains shorthand or duplicate fields".to_owned(),
            );
        }
    }
    if fields.len() != 13 {
        return Err("Snapping Drake definition fields differ from CardDefinition".to_owned());
    }
    check_string(fields["id"], "RAV-SNAPPING-DRAKE")?;
    check_string(fields["name"], "Snapping Drake")?;
    check_string(fields["set_code"], "RAV")?;
    check_drake_mana_cost(fields["mana_cost"])?;
    check_set(fields["colors"], &[&["Color", "Blue"]])?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_i16(fields["power"])?;
    check_some_i16(fields["toughness"])?;
    check_drake_keywords(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_snapping_drake::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SNAPPING-DRAKE")
        .ok_or_else(|| {
            "RAV-SNAPPING-DRAKE is missing from the executable card registry".to_owned()
        })?;
    if submitted.id != runtime.id
        || submitted.name != "Snapping Drake"
        || submitted.set_code != "RAV"
        || submitted.colors != BTreeSet::from([Color::Blue])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || !submitted.effects.is_empty()
    {
        return Err("RAV-SNAPPING-DRAKE identity or card frame differs".to_owned());
    }
    Ok(())
}

/// Rebuild the full RAV rules registry with the validated candidate definition
/// substituted for Snapping Drake. This makes the cast and state trace exercise the
/// submitted characteristics rather than a hard-coded reference definition.
fn new_rav_game_with_snapping_drake(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-SNAPPING-DRAKE")
        .ok_or_else(|| {
            "RAV-SNAPPING-DRAKE is missing from the executable card registry".to_owned()
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
        Err(format!("Snapping Drake string field must be {expected:?}"))
    }
}

fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Snapping Drake boolean field differs".to_owned())
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
        .ok_or_else(|| "Snapping Drake color set must be a literal array".to_owned())?;
    if items.len() != expected.len()
        || !items
            .iter()
            .zip(expected)
            .all(|(item, path)| check_path(item, path))
    {
        return Err("Snapping Drake color values differ".to_owned());
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
        .ok_or_else(|| "Snapping Drake set must use BTreeSet::from".to_owned())?;
    if args.len() != 1 {
        return Err("Snapping Drake set constructor arity differs".to_owned());
    }
    check_color_array(args[0], expected)
}

fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Snapping Drake mana colors must be an empty set".to_owned())
    }
}

/// The AST gate checks that submitted costs use only the card's printed blue
/// mana grammar. The exact total cost is asserted after the in-game payment
/// receipt, so a semantic cost mutation reaches casting and the trace oracle.
fn check_drake_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"])
        .ok_or_else(|| "Snapping Drake mana cost must use ManaCost::with_colors".to_owned())?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= 8)))
    {
        return Err(
            "Snapping Drake mana cost generic component must be a small literal".to_owned(),
        );
    }
    check_color_array(args[1], &[&["Color", "Blue"]])
}

fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Snapping Drake supported rules must be a static string slice".to_owned());
    };
    if reference.attrs.len() != 0 || reference.mutability.is_some() {
        return Err("Snapping Drake supported rules reference differs".to_owned());
    }
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Snapping Drake supported rules must be an array".to_owned())?;
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "flying",
    ];
    if items.len() != expected.len() {
        return Err("Snapping Drake supported rules differ".to_owned());
    }
    for (item, expected) in items.into_iter().zip(expected) {
        check_string(item, expected)?;
    }
    Ok(())
}

fn check_some_i16(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["Some"])
        .ok_or_else(|| "Snapping Drake power and toughness must be fixed literals".to_owned())?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= i16::MAX as u64)))
    {
        Ok(())
    } else {
        Err("Snapping Drake power or toughness differs".to_owned())
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
        _ => Err("Snapping Drake keyword and effect lists must be empty".to_owned()),
    }
}

fn check_drake_keywords(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Snapping Drake keywords must use a literal vec!".to_owned());
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return Err("Snapping Drake keywords must use a literal vec!".to_owned());
    }
    if mac.mac.tokens.is_empty() {
        return Ok(());
    }
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    let items = parser
        .parse2(mac.mac.tokens.clone())
        .map_err(|_| "Snapping Drake keyword list is not a literal array".to_owned())?;
    if items.len() == 1 && check_path(&items[0], &["Keyword", "Flying"]) {
        Ok(())
    } else {
        Err("Snapping Drake keyword list differs".to_owned())
    }
}

#[derive(Clone, Copy)]
enum Mutation {
    Power,
    Toughness,
    Flying,
    Cost,
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut mutated = candidate_snapping_drake::definition();
    match mutation {
        Mutation::Power => mutated.power = Some(2),
        Mutation::Toughness => mutated.toughness = Some(3),
        Mutation::Flying => mutated.keywords.clear(),
        Mutation::Cost => mutated.mana_cost.generic = 4,
    }
    mutated
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let candidate = mutated_definition(mutation);
    let captured = run_match_with(candidate, seed)?;
    let expected_diagnostic = match mutation {
        Mutation::Power => "resolved Snapping Drake is not a 3/2",
        Mutation::Toughness => "resolved Snapping Drake is not a 3/2",
        Mutation::Flying => "resolved Snapping Drake is missing Flying",
        Mutation::Cost => "paid 5 blue mana; expected 4",
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
    snapping_drake: Option<ObjectId>,
    power_toughness: Option<(i32, i32)>,
    has_flying: Option<bool>,
    mana_paid_blue: u8,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_SNAPPING_DRAKE_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SNAPPING_DRAKE_SEED must be an unsigned integer".to_owned())?;
        return Ok((seed, run_match(seed)?));
    }
    for seed in 0..=MAX_TRACE_SEED {
        // Seed selection is a short shuffle/opening-hand probe. Avoid replaying
        // dozens of complete games merely because no pilot card was drawn.
        if !opening_hand_has_drake(seed)? {
            continue;
        }
        if let Ok(captured) = run_match(seed) {
            if captured.terminal == "Winner(PlayerId(0))"
                && captured.winner == Some(0)
                && captured.rejected_moves == 0
                && captured.snapping_drake.is_some()
                && captured.mana_paid_blue == 4
            {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no clean Snapping Drake trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn task_deck() -> DeckList {
    DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 56,
            },
            DeckEntry {
                card: "RAV-SNAPPING-DRAKE".to_owned(),
                count: 4,
            },
        ],
        sideboard: vec![],
    }
}

fn opening_hand_has_drake(seed: u64) -> Result<bool, String> {
    let mut game = Game::new(card_definitions(), 2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &task_deck())
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    Ok(view
        .hand
        .iter()
        .any(|card| card.definition == Some("RAV-SNAPPING-DRAKE")))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with(candidate_snapping_drake::definition(), seed)
}

fn run_match_with(definition: CardDefinition, seed: u64) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let deck1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Boros aggro deck missing")?;
    // This task-local 60-card list guarantees copies of the pilot card while
    // remaining an ordinary constructed deck loaded into the real game.
    let deck0 = task_deck();
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
    let mut game = new_rav_game_with_snapping_drake(definition)?;
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
        Box::new(SnappingDrakePolicy::new(PlayerId(0), policy_cost)),
        Box::new(PassPolicy),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut resolved_characteristics = None;
    let mut has_flying = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Snapping Drake game hit a development run bound".to_owned());
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
                    "policy move rejected at turn {} step {:?} player {:?}/active {:?}/priority {:?}/stack {} with pool {:?}, hand {:?}, for {action_debug}: {error}",
                    game.turn,
                    view.step,
                    view.player,
                    view.active_player,
                    view.priority,
                    view.stack_depth,
                    view.mana_pool,
                    view.hand.iter().map(|card| card.definition).collect::<Vec<_>>(),
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if resolved_characteristics.is_none() {
            if let Some(object) = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == "RAV-SNAPPING-DRAKE") =>
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
                        .ok_or("Snapping Drake has no live power")?,
                    characteristics
                        .toughness
                        .ok_or("Snapping Drake has no live toughness")?,
                ));
                has_flying = Some(characteristics.keywords.contains(&Keyword::Flying));
            }
        }
    }
    let events = game.event_log.clone();
    let snapping_drake = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == "RAV-SNAPPING-DRAKE") =>
        {
            Some(*card)
        }
        _ => None,
    });
    if snapping_drake.is_none() {
        return Err(format!(
            "Snapping Drake was not cast; terminal={}, winner={:?}, turns={}, accepted_moves={accepted}",
            if game.winner().is_some() {
                format!("Winner(PlayerId({}))", game.winner().unwrap().0)
            } else {
                "Draw".to_owned()
            },
            game.winner().map(|player| player.0),
            game.turn,
        ));
    }
    let power_toughness = resolved_characteristics;
    let mut mana_paid_blue = 0_u8;
    if let Some(object) = snapping_drake {
        for event in &events {
            if let GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } = event
                && *card == object
            {
                mana_paid_blue = colors
                    .iter()
                    .filter(|color| **color == Color::Blue)
                    .count()
                    .try_into()
                    .unwrap_or(u8::MAX);
            }
        }
    }
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
        snapping_drake,
        power_toughness,
        has_flying,
        mana_paid_blue,
    })
}

struct SnappingDrakePolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    cost: ManaCost,
}

struct PassPolicy;

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.snapping_drake-pass-opponent.v1"
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

impl SnappingDrakePolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            cost,
        }
    }
}

impl CodePolicy for SnappingDrakePolicy {
    fn id(&self) -> &'static str {
        "rav.snapping_drake-cast.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.active_player == self.player
            && view.step == Step::DeclareAttackers
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some("RAV-SNAPPING-DRAKE") && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
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
                .find(|card| card.definition == Some("RAV-SNAPPING-DRAKE"))
            {
                let required_total = self.cost.mana_value();
                let blue = view.mana_pool.amount(Color::Blue) >= required_total;
                if blue && view.mana_pool.total() >= required_total {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::Blue; usize::from(self.cost.generic)],
                            hybrid: vec![],
                        },
                    };
                }
                let needed = (!blue).then_some(Color::Blue);
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
                return PolicyAction::PassPriority;
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
        return Err("Snapping Drake game did not reach a clean seat-zero terminal win".to_owned());
    }
    let object = captured
        .snapping_drake
        .ok_or("Snapping Drake was not cast by seat zero")?;
    if captured.power_toughness != Some((3, 2)) {
        return Err(format!(
            "resolved Snapping Drake is not a 3/2 (observed {:?})",
            captured.power_toughness
        ));
    }
    if captured.mana_paid_blue != 4 {
        return Err(format!(
            "Snapping Drake paid {} blue mana; expected 4",
            captured.mana_paid_blue
        ));
    }
    if captured.has_flying != Some(true) {
        return Err("resolved Snapping Drake is missing Flying".to_owned());
    }
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == object)).ok_or("Snapping Drake SpellCast missing")?;
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == object))
        .ok_or("Snapping Drake SpellResolved missing")?;
    let battlefield = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == object)).ok_or("Snapping Drake battlefield move missing")?;
    let attack = captured
        .events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&object))
        })
        .ok_or("Snapping Drake did not attack in the PvP trace")?;
    let cast_incarnation = captured
        .events
        .iter()
        .enumerate()
        .skip(cast + 1)
        .find_map(|(index, event)| matches!(event, GameEvent::ObjectIncarnationAdvanced { object: card, .. } if *card == object).then_some(index))
        .ok_or("Snapping Drake stack incarnation receipt missing")?;
    if !(cast < cast_incarnation
        && cast_incarnation < resolved
        && resolved < battlefield
        && battlefield < attack)
    {
        return Err("Snapping Drake cast, resolution, and entry order differs".to_owned());
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
            "Snapping Drake projected event sequence differs: {kinds:?}"
        ));
    }
    let resolved_event = relevant
        .iter()
        .find(|event| event.kind == "SpellResolved")
        .ok_or("Snapping Drake projected resolution missing")?;
    let attack_event = relevant
        .iter()
        .find(|event| event.kind == "AttackersDeclared")
        .ok_or("Snapping Drake projected attack missing")?;
    if attack_event.turn <= resolved_event.turn {
        return Err("Snapping Drake attacked before summoning sickness expired".to_owned());
    }
    Ok(json!({
        "id": "rav-snapping-drake-colored-creature-cast",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.snapping-drake-cast.v1", "rav.snapping-drake-pass-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "snapping_drake_object": object.0,
        "mana_paid": {"blue": captured.mana_paid_blue},
        "resolved_characteristics": {"power": captured.power_toughness.unwrap().0, "toughness": captured.power_toughness.unwrap().1, "flying": captured.has_flying.unwrap()},
        "entry_turn": resolved_event.turn,
        "attack_turn": attack_event.turn,
        "ordered_event_sequence": [cast, resolved, battlefield, attack],
        "stack_incarnation_sequence": cast_incarnation,
        "events": relevant,
        "passed": true
    }))
}

fn project_relevant(events: &[GameEvent], object: ObjectId) -> Vec<ProjectedEvent> {
    let mut attack_seen = false;
    project_events(events)
        .into_iter()
        .filter(|event| {
            if event.kind == "GameEnded" {
                return true;
            }
            if event.kind == "AttackersDeclared"
                && event.facts.objects.iter().any(|card| card.0 == object.0)
            {
                if attack_seen {
                    return false;
                }
                attack_seen = true;
                return true;
            }
            matches!(
                event.kind.as_str(),
                "CardMoved" | "SpellCast" | "SpellResolved"
            ) && event
                .facts
                .objects
                .first()
                .is_some_and(|card| card.0 == object.0)
                && (event.kind != "CardMoved" || event.facts.zone.as_deref() == Some("Battlefield"))
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
    let passed = definition_id == Some("RAV-SNAPPING-DRAKE");
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
