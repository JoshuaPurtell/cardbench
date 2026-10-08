//! Full-game RAV Selesnya Evangel trace authority for per-card set-PvP work.
//!
//! A legal constructed match casts Selesnya Evangel, activates its stack-using
//! ability with both tap costs, and verifies the resulting live Saproling.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, Color, CreatureSubtype,
    DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, TokenSpec, Zone,
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
const SELESNYA_EVANGEL_SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_evangel.rs";
const DECK_P0: &str = "rav_selesnya_evangel_green_white_curve";
const DECK_P1: &str = "rav_selesnya_evangel_basic_land_opponent";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/selesnya_evangel.rs"]
mod candidate_selesnya_evangel;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Selesnya Evangel PvP result: {error}");
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
            eprintln!("Selesnya Evangel PvP authority failed: {error}");
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
        || game.evangel_object != replay.evangel_object
        || game.token_object != replay.token_object
        || game.source_tapped_on_activation != replay.source_tapped_on_activation
        || game.helper_tapped_on_activation != replay.helper_tapped_on_activation
    {
        return Err("Selesnya Evangel full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "source_tap_cost_removed_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::SourceTap)?,
        "additional_creature_tap_cost_removed_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::AdditionalCreatureTap)?,
        "saproling_effect_removed_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::TokenEffect)?,
        "green_white_cast_cost_mutation_detected": mutation_reaches_trace_and_fails(seed, Mutation::WhiteCastCost)?,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["source_tap_cost_removed_mutation_detected"] != true
        || mutation_sensitivity["additional_creature_tap_cost_removed_mutation_detected"] != true
        || mutation_sensitivity["saproling_effect_removed_mutation_detected"] != true
        || mutation_sensitivity["green_white_cast_cost_mutation_detected"] != true
    {
        return Err(format!(
            "a Selesnya Evangel definition mutation escaped the trace oracle: {mutation_sensitivity}"
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
        "card": {"collector_number": 228, "definition_id": "RAV-SELESNYA-EVANGEL", "name": "Selesnya Evangel"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-SELESNYA-EVANGEL"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: green-white identity and colored payment",
                "106.1 and 106.4: mana pool and payment",
                "117.1b and 117.3c: priority to activate and priority after activation",
                "302.1-302.6: creature permanents and summoning sickness",
                "601.2 and 608.3a: casting and resolving a creature spell",
                "602.1-602.2: activated-ability costs and stack activation",
                "111.3: token characteristics"
            ],
            "source_contract": "RAV printed green-white 1/2 Selesnya Evangel with stack-backed Saproling creation",
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

/// Restricts submitted Rust to literal-only identity and ability definitions.
/// Validation happens before either submitted function is called.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SELESNYA_EVANGEL_SOURCE_PATH)
        .map_err(|error| format!("could not read Selesnya Evangel source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Selesnya Evangel source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Selesnya Evangel source contains an unsupported module attribute".to_owned());
    }

    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TokenSpec"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut definition = None;
    let mut ability = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Selesnya Evangel source contains an unsupported import".to_owned());
                };
                actual_imports.extend(paths);
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item);
            }
            Item::Fn(item) if item.sig.ident == "activated_ability" && ability.is_none() => {
                ability = Some(item);
            }
            _ => {
                return Err("Selesnya Evangel source contains executable top-level code".to_owned());
            }
        }
    }
    if actual_imports != expected_imports || file.items.len() != 4 {
        return Err(
            "Selesnya Evangel imports or item structure differ from the task contract".to_owned(),
        );
    }
    let function =
        definition.ok_or_else(|| "Selesnya Evangel definition() is missing".to_owned())?;
    let ability =
        ability.ok_or_else(|| "Selesnya Evangel activated_ability() is missing".to_owned())?;
    validate_definition_function(function)?;
    validate_ability_function(ability)
}

fn validate_definition_function(function: &syn::ItemFn) -> Result<(), String> {
    let card = direct_struct_function(function, "definition", "CardDefinition")?;
    let fields = literal_fields(card)?;
    if fields.len() != 13 {
        return Err("Selesnya Evangel definition fields differ from CardDefinition".to_owned());
    }
    check_string(fields["id"], "RAV-SELESNYA-EVANGEL")?;
    check_string(fields["name"], "Selesnya Evangel")?;
    check_string(fields["set_code"], "RAV")?;
    check_selesnya_mana_cost(fields["mana_cost"])?;
    check_set(
        fields["colors"],
        &[&["Color", "Green"], &["Color", "White"]],
    )?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(
        fields["supported_rules"],
        &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-token-creation-with-creature-tap-cost",
        ],
    )?;
    check_some_i16_equals(fields["power"], 1)?;
    check_some_i16_equals(fields["toughness"], 2)?;
    check_empty_vec(fields["effects"])?;
    check_empty_vec(fields["keywords"])?;
    Ok(())
}

fn validate_ability_function(function: &syn::ItemFn) -> Result<(), String> {
    let ability = direct_struct_function(function, "activated_ability", "ActivatedAbility")?;
    let fields = literal_fields(ability)?;
    if fields.len() != 11 {
        return Err("Selesnya Evangel ability fields differ from ActivatedAbility".to_owned());
    }
    check_string(fields["id"], "create-saproling")?;
    check_generic_mana_cost(fields["mana_cost"], 1)?;
    check_literal_bool(fields["tap_cost"])?;
    check_bool(fields["sorcery_speed"], false)?;
    check_literal_u8_at_most(fields["additional_tap_creatures"], 1)?;
    check_bool(fields["sacrifice_source"], false)?;
    check_literal_u8(fields["sacrifice_creatures"], 0)?;
    check_literal_u8(fields["sacrifice_lands"], 0)?;
    check_literal_u8(fields["discard_cards"], 0)?;
    check_empty_vec(fields["targets"])?;
    check_saproling_effect_or_empty(fields["effects"])
}

fn direct_struct_function<'a>(
    function: &'a syn::ItemFn,
    name: &str,
    return_type: &str,
) -> Result<&'a syn::ExprStruct, String> {
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
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &[return_type]))
        || function.block.stmts.len() != 1
    {
        return Err(format!(
            "Selesnya Evangel {name} has an unsupported function signature"
        ));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Selesnya Evangel {name} must return one direct literal"
        ));
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err(format!("Selesnya Evangel {name} literal type differs"));
    }
    Ok(value)
}

fn literal_fields(
    value: &syn::ExprStruct,
) -> Result<std::collections::BTreeMap<String, &Expr>, String> {
    let mut fields = std::collections::BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Selesnya Evangel literal fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Selesnya Evangel literal has shorthand or duplicate fields".to_owned());
        }
    }
    Ok(fields)
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_selesnya_evangel::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-EVANGEL")
        .ok_or_else(|| "RAV-SELESNYA-EVANGEL is missing from the executable registry".to_owned())?;
    if submitted.id != registered.id
        || submitted.name != "Selesnya Evangel"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(0, [Color::Green, Color::White])
        || submitted.colors != BTreeSet::from([Color::Green, Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(1)
        || submitted.toughness != Some(2)
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
    {
        return Err("RAV-SELESNYA-EVANGEL identity or printed characteristics differ".to_owned());
    }
    let ability = candidate_selesnya_evangel::activated_ability();
    if ability.id != "create-saproling"
        || ability.mana_cost != ManaCost::new(1)
        || ability.sorcery_speed
        || ability.additional_tap_creatures > 1
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || !(ability.effects.is_empty()
            || ability.effects
                == [Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }])
    {
        return Err("RAV-SELESNYA-EVANGEL activated ability identity or cost differs".to_owned());
    }
    Ok(())
}

/// Rebuild the full RAV rules registry with the validated candidate definition
/// substituted for Selesnya Evangel. This makes the cast and state trace exercise the
/// submitted characteristics rather than a hard-coded reference definition.
fn new_rav_game_with_selesnya_evangel(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-SELESNYA-EVANGEL")
        .ok_or_else(|| {
            "RAV-SELESNYA-EVANGEL is missing from the executable card registry".to_owned()
        })?;
    *registered = definition;

    let mut abilities = rav_activated_ability_bindings();
    let registered_ability = abilities
        .iter_mut()
        .find(|binding| {
            binding.card_definition == "RAV-SELESNYA-EVANGEL"
                && binding.ability.id == "create-saproling"
        })
        .ok_or_else(|| "RAV Selesnya Evangel activated-ability binding is missing".to_owned())?;
    registered_ability.ability = ability;

    let mut triggers = rav_triggered_ability_bindings();
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
            "Selesnya Evangel string field must be {expected:?}"
        ))
    }
}

fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Selesnya Evangel boolean field differs".to_owned())
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
        .ok_or_else(|| "Selesnya Evangel color set must be a literal array".to_owned())?;
    if items.len() != expected.len()
        || !items
            .iter()
            .zip(expected)
            .all(|(item, path)| check_path(item, path))
    {
        return Err("Selesnya Evangel color values differ".to_owned());
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
        .ok_or_else(|| "Selesnya Evangel set must use BTreeSet::from".to_owned())?;
    if args.len() != 1 {
        return Err("Selesnya Evangel set constructor arity differs".to_owned());
    }
    check_color_array(args[0], expected)
}

fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Selesnya Evangel mana colors must be an empty set".to_owned())
    }
}

fn check_selesnya_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"])
        .ok_or_else(|| "Selesnya Evangel mana cost must use ManaCost::with_colors".to_owned())?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok().is_some_and(|number| number <= 8)))
    {
        return Err(
            "Selesnya Evangel mana cost generic component must be a small literal".to_owned(),
        );
    }
    check_color_array(args[1], &[&["Color", "Green"], &["Color", "White"]])
}

fn check_generic_mana_cost(expr: &Expr, expected: u64) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "new"])
        .ok_or_else(|| "Selesnya Evangel ability mana cost must use ManaCost::new".to_owned())?;
    if args.len() != 1
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
    {
        return Err("Selesnya Evangel activated ability generic cost differs".to_owned());
    }
    Ok(())
}

fn check_literal_bool(expr: &Expr) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(_)))
    {
        Ok(())
    } else {
        Err("Selesnya Evangel tap cost must be a boolean literal".to_owned())
    }
}

fn check_literal_u8(expr: &Expr, expected: u64) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("Selesnya Evangel activated ability cost field differs".to_owned())
    }
}

fn check_literal_u8_at_most(expr: &Expr, maximum: u64) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().is_ok_and(|number| number <= maximum)))
    {
        Ok(())
    } else {
        Err(
            "Selesnya Evangel additional creature tap cost must be a literal zero or one"
                .to_owned(),
        )
    }
}

fn check_rules(expr: &Expr, expected: &[&str]) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Selesnya Evangel supported rules must be a static string slice".to_owned());
    };
    if reference.attrs.len() != 0 || reference.mutability.is_some() {
        return Err("Selesnya Evangel supported rules reference differs".to_owned());
    }
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Selesnya Evangel supported rules must be an array".to_owned())?;
    if items.len() != expected.len() {
        return Err("Selesnya Evangel supported rules differ".to_owned());
    }
    for (item, expected) in items.into_iter().zip(expected) {
        check_string(item, expected)?;
    }
    Ok(())
}

fn check_some_i16_equals(expr: &Expr, expected: i16) -> Result<(), String> {
    let args = call_expr(expr, &["Some"])
        .ok_or_else(|| "Selesnya Evangel power and toughness must be fixed literals".to_owned())?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<i16>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("Selesnya Evangel power or toughness differs".to_owned())
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
        _ => Err("Selesnya Evangel keyword and effect lists must be empty".to_owned()),
    }
}

fn check_saproling_effect_or_empty(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Selesnya Evangel effect list must use a literal vec!".to_owned());
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return Err("Selesnya Evangel effect list must use a literal vec!".to_owned());
    }
    if mac.mac.tokens.is_empty() {
        return Ok(());
    }
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    let items = parser
        .parse2(mac.mac.tokens.clone())
        .map_err(|_| "Selesnya Evangel effects are not a literal array".to_owned())?;
    if items.len() != 1 {
        return Err("Selesnya Evangel must have zero or one token-creation effect".to_owned());
    }
    let Expr::Struct(effect) = &items[0] else {
        return Err("Selesnya Evangel effect must be one direct literal".to_owned());
    };
    if !path_is(&effect.path, &["Effect", "CreateToken"])
        || effect.rest.is_some()
        || !effect.attrs.is_empty()
    {
        return Err("Selesnya Evangel effect must be the token-creation operation".to_owned());
    }
    let fields = literal_fields(effect)?;
    if fields.len() != 2
        || call_expr(fields["token"], &["TokenSpec", "saproling"])
            .is_none_or(|args| !args.is_empty())
    {
        return Err("Selesnya Evangel token specification differs".to_owned());
    }
    check_literal_u8(fields["count"], 1)
}

#[derive(Clone, Copy)]
enum Mutation {
    SourceTap,
    AdditionalCreatureTap,
    TokenEffect,
    WhiteCastCost,
}

fn mutated_candidate(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_selesnya_evangel::definition();
    let mut ability = candidate_selesnya_evangel::activated_ability();
    match mutation {
        Mutation::SourceTap => ability.tap_cost = false,
        Mutation::AdditionalCreatureTap => ability.additional_tap_creatures = 0,
        Mutation::TokenEffect => ability.effects.clear(),
        Mutation::WhiteCastCost => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Green]);
        }
    }
    (definition, ability)
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let (definition, ability) = mutated_candidate(mutation);
    let captured = run_match_with(definition, ability, seed, false)?;
    let expected = match mutation {
        Mutation::SourceTap => "Selesnya Evangel source tap cost was not paid",
        Mutation::AdditionalCreatureTap => {
            "Selesnya Evangel additional creature tap cost was not paid"
        }
        Mutation::TokenEffect => {
            "Selesnya Evangel activated ability did not create a green 1/1 Saproling"
        }
        Mutation::WhiteCastCost => "Selesnya Evangel cast payment did not include green and white",
    };
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains(expected)))
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    evangel_object: Option<ObjectId>,
    helper_object: Option<ObjectId>,
    ability_mana_cost: Option<ManaCost>,
    source_tapped_on_activation: bool,
    helper_tapped_on_activation: bool,
    token_object: Option<ObjectId>,
    token_spec: Option<TokenSpec>,
    token_zone: Option<Zone>,
    token_controller: Option<usize>,
    token_is_green_creature_saproling: bool,
    token_power_toughness: Option<(i32, i32)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SELESNYA_EVANGEL_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_SELESNYA_EVANGEL_SEED must be an unsigned integer".to_owned()
        })?;
        return Ok((seed, run_match(seed)?));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(preview) = run_match_preview(seed)
            && preview.evangel_object.is_some()
            && (candidate_selesnya_evangel::activated_ability().additional_tap_creatures == 0
                || preview.helper_object.is_some())
            && preview.ability_mana_cost.as_ref() == Some(&ManaCost::new(1))
        {
            // Seed discovery identifies the activation path only. Semantic
            // payment, tap-cost, and token assertions stay in the trace oracle
            // so their negative controls fail quickly on the same seed.
            return Ok((seed, run_match(seed)?));
        }
    }
    Err(format!(
        "no complete Selesnya Evangel activation trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with(
        candidate_selesnya_evangel::definition(),
        candidate_selesnya_evangel::activated_ability(),
        seed,
        false,
    )
}

fn run_match_preview(seed: u64) -> Result<CapturedMatch, String> {
    run_match_with(
        candidate_selesnya_evangel::definition(),
        candidate_selesnya_evangel::activated_ability(),
        seed,
        true,
    )
}

fn run_match_with(
    definition: CardDefinition,
    ability: ActivatedAbility,
    seed: u64,
    stop_after_activation: bool,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-SELESNYA-EVANGEL".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-GLASS-GOLEM".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
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
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [("seat zero", &deck0), ("seat one", &deck1)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!(
                "{label} deck has {count} cards; expected a legal 60-card deck"
            ));
        }
    }
    let _ = decks; // The complete RAV card registry is supplied by the set module.
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let policy_cost = definition.mana_cost.clone();
    let policy_additional_tap_creatures = ability.additional_tap_creatures;
    let mut game = new_rav_game_with_selesnya_evangel(definition, ability)?;
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
        Box::new(EvangelPolicy::new(
            PlayerId(0),
            policy_cost,
            policy_additional_tap_creatures,
        )),
        Box::new(PassPolicy),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut source_tapped_on_activation = false;
    let mut helper_tapped_on_activation = false;
    let mut helper_object = None;
    let mut activation_seen = false;

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Selesnya Evangel game hit a development run bound".to_owned());
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
        let activation = match &action {
            PolicyAction::ActivateAbility { activation } => Some((
                activation.source,
                activation.additional_tap_creatures.first().copied(),
            )),
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?} with pool {:?}: {error}",
                    view.turn, view.step, view.mana_pool,
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if let Some((source, helper)) = activation
            && !activation_seen
        {
            activation_seen = true;
            helper_object = helper;
            source_tapped_on_activation = game.object(source).is_ok_and(|object| object.tapped);
            helper_tapped_on_activation =
                helper.is_some_and(|helper| game.object(helper).is_ok_and(|object| object.tapped));
        }
        if stop_after_activation && activation_seen {
            let activation_source = game.event_log.iter().find_map(|event| match event {
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    definition: "RAV-SELESNYA-EVANGEL",
                    ability: "create-saproling",
                    ..
                } => Some(*source),
                _ => None,
            });
            if activation_source.is_some_and(|source| {
                game.event_log.iter().any(|event| {
                    matches!(
                        event,
                        GameEvent::AbilityResolved {
                            source: resolved_source,
                            ability: "create-saproling",
                            ..
                        } if *resolved_source == source
                    )
                })
            }) {
                break;
            }
        }
    }

    let events = game.event_log.clone();
    let evangel_object = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|card| card.id == "RAV-SELESNYA-EVANGEL") =>
        {
            Some(*card)
        }
        _ => None,
    });
    let ability_activation = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                definition,
                ability: "create-saproling",
                ..
            } if *definition == "RAV-SELESNYA-EVANGEL" => Some((index, *source)),
            _ => None,
        });
    if let Some((_, source)) = ability_activation {
        helper_object = events.iter().find_map(|event| match event {
            GameEvent::AdditionalCreatureTappedAsAbilityCost {
                player: PlayerId(0),
                source: event_source,
                permanent,
            } if *event_source == source => Some(*permanent),
            _ => None,
        });
    }
    let ability_mana_cost = ability_activation.and_then(|(_, source)| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source: event_source,
                ability: "create-saproling",
                mana_cost,
            } if *event_source == source => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let token_object = ability_activation.and_then(|(activation_index, source)| {
        events
            .iter()
            .enumerate()
            .skip(activation_index + 1)
            .find_map(|(_, event)| match event {
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token,
                } => Some(*token),
                GameEvent::AbilityResolved {
                    source: resolved_source,
                    ability: "create-saproling",
                    ..
                } if *resolved_source == source => None,
                _ => None,
            })
    });
    let token_spec = token_object.and_then(|token| game.object(token).ok()?.token.clone());
    let token_zone = token_object.and_then(|token| game.zone_of(token));
    let token_controller =
        token_object.and_then(|token| game.controller_of(token).ok().map(|player| player.0));
    let token_is_green_creature_saproling = token_object.is_some_and(|token| {
        game.object(token)
            .is_ok_and(|object| object.token.as_ref() == Some(&TokenSpec::saproling()))
            && game.characteristics(token).is_ok_and(|characteristics| {
                characteristics.colors == BTreeSet::from([Color::Green])
                    && characteristics.card_types.contains(&CardType::Creature)
                    && characteristics
                        .creature_subtypes
                        .contains(&CreatureSubtype::Saproling)
            })
    });
    let token_power_toughness = token_object.and_then(|token| {
        let characteristics = game.characteristics(token).ok()?;
        Some((characteristics.power?, characteristics.toughness?))
    });
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: if stop_after_activation && !game.is_game_over() {
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
        evangel_object,
        helper_object,
        ability_mana_cost,
        source_tapped_on_activation,
        helper_tapped_on_activation,
        token_object,
        token_spec,
        token_zone,
        token_controller,
        token_is_green_creature_saproling,
        token_power_toughness,
    })
}

struct EvangelPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    cost: ManaCost,
    additional_tap_creatures: u8,
    activated_once: bool,
}

struct PassPolicy;

impl EvangelPolicy {
    fn new(player: PlayerId, cost: ManaCost, additional_tap_creatures: u8) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            cost,
            additional_tap_creatures,
            activated_once: false,
        }
    }

    fn payment(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for colored in &cost.colored {
            let (_, amount) = available.iter_mut().find(|(color, _)| color == colored)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::with_capacity(usize::from(cost.generic));
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

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        (view.lands_played == 0)
            .then(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .flatten()
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn missing_color(&self, view: &GameView, cost: &ManaCost) -> Option<Color> {
        for color in &cost.colored {
            if view.mana_pool.amount(*color) == 0 {
                return Some(*color);
            }
        }
        None
    }

    fn make_mana(&self, view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let preferred = preferred.unwrap_or(Color::Green);
        let land = view
            .own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && !land.mana_colors.is_empty()
                })
            })?;
        let color = if land.mana_colors.contains(&preferred) {
            preferred
        } else {
            *land.mana_colors.iter().next()?
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn creature_cost(&self, card: &cardbench_magic_engine::CardView) -> Option<ManaCost> {
        match card.definition {
            Some("RAV-SELESNYA-EVANGEL") => Some(self.cost.clone()),
            Some("RAV-GLASS-GOLEM") => Some(ManaCost::with_colors(0, [Color::Green, Color::White])),
            _ => None,
        }
    }
}

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya_evangel-pass-opponent.v1"
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

impl CodePolicy for EvangelPolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya_evangel-cast-activate-and-attack.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player
            || view.decision_player != self.player
            || view.active_player != self.player
            || view.priority != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step != Step::PrecombatMain || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        let evangel = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some("RAV-SELESNYA-EVANGEL"));
        if !self.activated_once
            && let Some(source) = evangel.filter(|card| !card.tapped && !card.summoning_sick)
            && view.mana_pool.total() >= 1
            && let Some(helper) = view.own_battlefield.iter().find(|card| {
                card.id != source.id
                    && card.card_types.contains(&CardType::Creature)
                    && !card.tapped
            })
        {
            self.activated_once = true;
            return PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source: source.id,
                    ability_id: "create-saproling",
                    sacrifice_sources: vec![],
                    additional_tap_creatures: if self.additional_tap_creatures == 0 {
                        vec![]
                    } else {
                        vec![helper.id]
                    },
                    discard_cards: vec![],
                    targets: vec![],
                },
            };
        }
        let mut creatures: Vec<_> = view
            .hand
            .iter()
            .filter_map(|card| self.creature_cost(card).map(|cost| (card, cost)))
            .collect();
        creatures.sort_by_key(|(card, _)| {
            if card.definition == Some("RAV-SELESNYA-EVANGEL") {
                0
            } else {
                1
            }
        });
        for (card, cost) in creatures {
            if let Some(mana_selection) = self.payment(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
        }
        let preferred = view
            .hand
            .iter()
            .find_map(|card| {
                self.creature_cost(card)
                    .and_then(|cost| self.missing_color(view, &cost))
            })
            .or_else(|| {
                (!self.activated_once && evangel.is_some() && view.mana_pool.total() == 0)
                    .then_some(Color::Green)
            });
        if let Some(action) = self.make_mana(view, preferred) {
            return action;
        }
        self.inner.propose_move(view)
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Selesnya Evangel game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if captured.rejected_moves != 0 {
        return Err(format!(
            "Selesnya Evangel trace rejected {} submitted moves",
            captured.rejected_moves
        ));
    }
    let evangel = captured
        .evangel_object
        .ok_or("Selesnya Evangel was not cast by seat zero")?;
    if captured.ability_mana_cost.as_ref() != Some(&ManaCost::new(1)) {
        return Err(format!(
            "Selesnya Evangel ability paid {:?}; expected generic 1",
            captured.ability_mana_cost
        ));
    }
    if !captured.source_tapped_on_activation {
        return Err("Selesnya Evangel source tap cost was not paid".to_owned());
    }
    if !captured.helper_tapped_on_activation {
        return Err("Selesnya Evangel additional creature tap cost was not paid".to_owned());
    }
    let helper = captured
        .helper_object
        .ok_or("Selesnya Evangel ability did not tap another creature")?;
    if helper == evangel {
        return Err("Selesnya Evangel paid its additional tap cost with itself".to_owned());
    }
    let token = captured
        .token_object
        .ok_or("Selesnya Evangel activated ability did not create a green 1/1 Saproling")?;
    if captured.token_spec.as_ref() != Some(&TokenSpec::saproling())
        || captured.token_zone != Some(Zone::Battlefield)
        || captured.token_controller != Some(0)
        || !captured.token_is_green_creature_saproling
        || captured.token_power_toughness != Some((1, 1))
    {
        return Err(
            "Selesnya Evangel token is not a live green 1/1 Saproling controlled by seat zero"
                .to_owned(),
        );
    }

    let payment = captured
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == evangel => Some(colors),
            _ => None,
        })
        .ok_or("Selesnya Evangel SpellManaPaid missing")?;
    let green = payment
        .iter()
        .filter(|color| **color == Color::Green)
        .count();
    let white = payment
        .iter()
        .filter(|color| **color == Color::White)
        .count();
    if green != 1 || white != 1 || payment.len() != 2 {
        return Err(format!(
            "Selesnya Evangel cast payment did not include green and white: {payment:?}"
        ));
    }
    let activation = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                definition,
                ability: "create-saproling",
                ..
            } if *source == evangel && *definition == "RAV-SELESNYA-EVANGEL" => Some(index),
            _ => None,
        })
        .ok_or("Selesnya Evangel AbilityActivated receipt missing")?;
    let token_sequence = captured.events.iter().enumerate().find_map(|(index, event)| {
        matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token: created } if *created == token)
            .then_some(index)
    }).ok_or("Selesnya Evangel TokenCreated receipt missing")?;
    let resolved = captured.events.iter().enumerate().find_map(|(index, event)| {
        matches!(event, GameEvent::AbilityResolved { source, ability: "create-saproling", .. } if *source == evangel)
            .then_some(index)
    }).ok_or("Selesnya Evangel AbilityResolved receipt missing")?;
    let ability_payment = captured.events.iter().enumerate().find_map(|(index, event)| {
        matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "create-saproling", mana_cost }
            if *source == evangel && *mana_cost == ManaCost::new(1))
            .then_some(index)
    }).ok_or("Selesnya Evangel ability mana payment receipt missing")?;
    let helper_tap = captured.events.iter().enumerate().find_map(|(index, event)| {
        matches!(event, GameEvent::AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source, permanent }
            if *source == evangel && *permanent == helper)
            .then_some(index)
    }).ok_or("Selesnya Evangel additional creature tap receipt missing")?;
    let cast = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == evangel)
                .then_some(index)
        })
        .ok_or("Selesnya Evangel SpellCast missing")?;
    let spell_resolved = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == evangel).then_some(index)
        })
        .ok_or("Selesnya Evangel SpellResolved missing")?;
    let entered = captured.events.iter().enumerate().find_map(|(index, event)| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == evangel).then_some(index)
    }).ok_or("Selesnya Evangel battlefield entry receipt missing")?;
    let ended = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
            .then_some(index)
        })
        .ok_or("Selesnya Evangel terminal GameEnded receipt missing")?;
    if !(cast < spell_resolved
        && spell_resolved < entered
        && ability_payment < helper_tap
        && helper_tap < activation
        && activation < token_sequence
        && token_sequence < resolved
        && resolved < ended)
    {
        return Err(
            "Selesnya Evangel cast, cost, activation, token, and resolution event order differs"
                .to_owned(),
        );
    }
    if !captured.events.iter().any(|event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == evangel)
    }) {
        return Err("Selesnya Evangel spell payment was not represented".to_owned());
    }
    let relevant = project_relevant(&captured.events, evangel, helper, token);
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityManaPaid",
        "AdditionalCreatureTappedAsAbilityCost",
        "AbilityActivated",
        "TokenCreated",
        "AbilityResolved",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Selesnya Evangel ordered event projection differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-selesnya-evangel-green-white-cast-activated-saproling",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.selesnya_evangel-cast-activate-and-attack.v1", "rav.selesnya_evangel-pass-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "selesnya_evangel_object": evangel.0,
        "helper_creature_object": helper.0,
        "cast_mana_paid": {"green": green, "white": white, "total": payment.len()},
        "ability_mana_cost": {"generic": 1},
        "activation_cost_state": {
            "source_tapped": captured.source_tapped_on_activation,
            "other_creature_tapped": captured.helper_tapped_on_activation
        },
        "saproling": {
            "object": token.0,
            "controller_seat": captured.token_controller,
            "zone": format!("{:?}", captured.token_zone.unwrap()),
            "green_creature_saproling": captured.token_is_green_creature_saproling,
            "power": captured.token_power_toughness.unwrap().0,
            "toughness": captured.token_power_toughness.unwrap().1
        },
        "ordered_event_sequence": relevant.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        "events": relevant,
        "passed": true
    }))
}

fn project_relevant(
    events: &[GameEvent],
    evangel: ObjectId,
    helper: ObjectId,
    token: ObjectId,
) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid" | "SpellCast" | "SpellResolved" => event
                .canonical
                .contains(&format!("card: ObjectId({})", evangel.0)),
            "CardMoved" => {
                event
                    .canonical
                    .contains(&format!("card: ObjectId({})", evangel.0))
                    && event.canonical.contains("to: Battlefield")
            }
            "AbilityManaPaid" | "AbilityActivated" | "AbilityResolved" => {
                event
                    .canonical
                    .contains(&format!("source: ObjectId({})", evangel.0))
                    && event.canonical.contains("create-saproling")
            }
            "AdditionalCreatureTappedAsAbilityCost" => {
                event
                    .canonical
                    .contains(&format!("source: ObjectId({})", evangel.0))
                    && event
                        .canonical
                        .contains(&format!("permanent: ObjectId({})", helper.0))
            }
            "TokenCreated" => event
                .canonical
                .contains(&format!("token: ObjectId({})", token.0)),
            "GameEnded" => true,
            _ => false,
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
    let passed = definition_id == Some("RAV-SELESNYA-EVANGEL");
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
