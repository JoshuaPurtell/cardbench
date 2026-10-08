//! Full-game RAV Boros Guildmage authority for its two targeted activations.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color,
    CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, HybridManaSymbol, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement,
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
const CARD_ID: &str = "RAV-BOROS-GUILDMAGE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_guildmage.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/boros_guildmage.rs"]
mod candidate_boros_guildmage;

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
            eprintln!("Boros Guildmage PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.guildmage != replay.guildmage
        || captured.post_haste_keywords != replay.post_haste_keywords
        || captured.post_first_strike_keywords != replay.post_first_strike_keywords
    {
        return Err("Boros Guildmage full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "haste_grant_changed_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::HasteEffect)?,
        "first_strike_grant_changed_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::FirstStrikeEffect)?,
        "red_haste_cost_increased_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::HasteCost)?,
        "white_first_strike_cost_increased_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::FirstStrikeCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "haste_grant_changed_detected_in_game",
        "first_strike_grant_changed_detected_in_game",
        "red_haste_cost_increased_detected_in_game",
        "white_first_strike_cost_increased_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Boros Guildmage mutation escaped the trace oracle: {key}"
            ));
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
        "card": {"collector_number": 242, "definition_id": CARD_ID, "name": "Boros Guildmage"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: red-white color identity and hybrid payment",
                "117: priority to activate an ability and priority after activation",
                "302.1-302.6: creature permanents, haste, and summoning sickness",
                "601.2 and 608.3a: casting and resolving a creature spell",
                "602.1-602.2: activating abilities, paying costs, and using the stack",
                "611.2a and 613: end-of-turn duration and continuous-effect layers",
                "508: declaring an attacker after gaining haste",
                "510.4 and 702.7: First Strike damage before normal combat damage"
            ],
            "source_contract": "RAV Boros Guildmage definition and candidate-owned red Haste and white First Strike activated bindings",
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
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace],
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
    let definition = candidate_boros_guildmage::definition();
    if definition.id != CARD_ID
        || definition.name != "Boros Guildmage"
        || definition.set_code != "RAV"
        || definition.mana_cost
            != ManaCost::with_hybrid(
                0,
                [],
                [
                    HybridManaSymbol {
                        first: Color::Red,
                        second: Color::White,
                    },
                    HybridManaSymbol {
                        first: Color::Red,
                        second: Color::White,
                    },
                ],
            )
        || definition.colors != BTreeSet::from([Color::Red, Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(2)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err(
            "Boros Guildmage printed definition differs from RAV collector #242".to_owned(),
        );
    }
    for (ability, id, color, keyword) in [
        (
            candidate_boros_guildmage::grant_haste_ability(),
            "grant-haste",
            Color::Red,
            Keyword::Haste,
        ),
        (
            candidate_boros_guildmage::grant_first_strike_ability(),
            "grant-first-strike",
            Color::White,
            Keyword::FirstStrike,
        ),
    ] {
        if ability.id != id
            || ability.mana_cost != ManaCost::with_colors(0, [color])
            || ability.tap_cost
            || ability.sorcery_speed
            || ability.additional_tap_creatures != 0
            || ability.sacrifice_source
            || ability.sacrifice_creatures != 0
            || ability.sacrifice_lands != 0
            || ability.discard_cards != 0
            || ability.targets != [TargetRequirement::Creature]
            || ability.effects != [Effect::ModifyTargetKeywordUntilEndOfTurn { keyword }]
        {
            return Err(format!(
                "Boros Guildmage {id} binding differs from its printed ability"
            ));
        }
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Boros Guildmage source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Boros Guildmage source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 5 {
        return Err("Boros Guildmage source has unsupported module items".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "HybridManaSymbol"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
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
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Boros Guildmage source has an unsupported import".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) => {
                if functions
                    .insert(function.sig.ident.to_string(), function)
                    .is_some()
                {
                    return Err("Boros Guildmage source repeats a function".to_owned());
                }
            }
            _ => return Err("Boros Guildmage source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected_imports || functions.len() != 3 {
        return Err(
            "Boros Guildmage imports or function set differ from the source contract".to_owned(),
        );
    }
    let definition = functions
        .get("definition")
        .ok_or("Boros Guildmage definition() is missing")?;
    let haste = functions
        .get("grant_haste_ability")
        .ok_or("Boros Guildmage Haste binding is missing")?;
    let first_strike = functions
        .get("grant_first_strike_ability")
        .ok_or("Boros Guildmage First Strike binding is missing")?;
    let card = direct_struct_function(definition, "CardDefinition")?;
    let fields = literal_fields(card)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Boros Guildmage")
        || !check_string(fields["set_code"], "RAV")
        || !check_hybrid_card_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Red"], &["Color", "White"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Creature"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-grant-haste",
                "activated-grant-first-strike",
            ],
        )
        || !check_some_i16(fields["power"], 2)
        || !check_some_i16(fields["toughness"], 2)
        || !check_empty_vec(fields["keywords"])
        || !check_empty_vec(fields["effects"])
    {
        return Err("Boros Guildmage definition is outside its literal source contract".to_owned());
    }
    validate_ability_function(haste, "grant-haste", "Red", "Haste")?;
    validate_ability_function(first_strike, "grant-first-strike", "White", "FirstStrike")
}

fn validate_ability_function(
    function: &syn::ItemFn,
    id: &str,
    color: &str,
    keyword: &str,
) -> Result<(), String> {
    let ability = direct_struct_function(function, "ActivatedAbility")?;
    let fields = literal_fields(ability)?;
    if fields.len() != 11
        || !check_string(fields["id"], id)
        || !check_colored_cost(fields["mana_cost"], color)
        || !check_bool(fields["tap_cost"], false)
        || !check_bool(fields["sorcery_speed"], false)
        || !check_u8(fields["additional_tap_creatures"], 0)
        || !check_bool(fields["sacrifice_source"], false)
        || !check_u8(fields["sacrifice_creatures"], 0)
        || !check_u8(fields["sacrifice_lands"], 0)
        || !check_u8(fields["discard_cards"], 0)
        || !check_vec_path(fields["targets"], &["TargetRequirement", "Creature"])
        || !check_keyword_effect(fields["effects"], keyword)
    {
        return Err(format!(
            "Boros Guildmage {id} ability is outside its literal source contract"
        ));
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
        return Err(format!(
            "Boros Guildmage function {} has an unsupported signature",
            function.sig.ident
        ));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Boros Guildmage function {} must return one literal",
            function.sig.ident
        ));
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err(format!(
            "Boros Guildmage function {} has the wrong literal type",
            function.sig.ident
        ));
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
        UseTree::Rename(rename) => {
            let mut path = prefix.to_vec();
            path.push(rename.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Glob(_) => false,
    }
}

fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(segment.arguments, syn::PathArguments::None)
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
        || call.attrs.len() > 0
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

fn check_u8(expr: &Expr, expected: u64) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
}

fn check_some_i16(expr: &Expr, expected: i16) -> bool {
    call_expr(expr, "Some", "_never") // keep the expression shape closed below
        .is_none()
        && matches!(expr, Expr::Call(call)
            if call.args.len() == 1
                && matches!(&*call.func, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["Some"]))
                && matches!(&call.args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<i16>().ok() == Some(expected))))
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
    array.elems.len() == expected.len()
        && array.elems.iter().zip(expected).all(|(value, path)| {
            matches!(value, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, path))
        })
}

fn check_hybrid_card_cost(expr: &Expr) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_hybrid") else {
        return false;
    };
    if args.len() != 3 || !check_u8(args[0], 0) {
        return false;
    }
    if !matches!(args[1], Expr::Array(array) if array.elems.is_empty()) {
        return false;
    }
    let Expr::Array(symbols) = args[2] else {
        return false;
    };
    symbols.elems.len() == 2 && symbols.elems.iter().all(check_red_white_hybrid_symbol)
}

fn check_red_white_hybrid_symbol(expr: &Expr) -> bool {
    let Expr::Struct(symbol) = expr else {
        return false;
    };
    if !path_is(&symbol.path, &["HybridManaSymbol"])
        || symbol.rest.is_some()
        || symbol.fields.len() != 2
    {
        return false;
    }
    let mut first = false;
    let mut second = false;
    for field in &symbol.fields {
        let syn::Member::Named(name) = &field.member else {
            return false;
        };
        if name == "first" {
            first = matches!(&field.expr, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["Color", "Red"]));
        } else if name == "second" {
            second = matches!(&field.expr, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["Color", "White"]));
        } else {
            return false;
        }
    }
    first && second
}

fn check_colored_cost(expr: &Expr, color: &str) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_colors") else {
        return false;
    };
    args.len() == 2
        && check_u8(args[0], 0)
        && matches!(args[1], Expr::Array(array) if array.elems.len() == 1
            && matches!(&array.elems[0], Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["Color", color])))
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
            .all(|(value, text)| check_string(value, text))
}

fn check_empty_vec(expr: &Expr) -> bool {
    matches!(expr, Expr::Macro(mac) if mac.attrs.is_empty() && mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty())
}

fn vec_items(expr: &Expr) -> Option<Vec<Expr>> {
    let Expr::Macro(mac) = expr else {
        return None;
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return None;
    }
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    parser
        .parse2(mac.mac.tokens.clone())
        .ok()
        .map(|items| items.into_iter().collect())
}

fn check_vec_path(expr: &Expr, path: &[&str]) -> bool {
    vec_items(expr).is_some_and(|items| items.len() == 1
        && matches!(&items[0], Expr::Path(value) if value.qself.is_none() && path_is(&value.path, path)))
}

fn check_keyword_effect(expr: &Expr, keyword: &str) -> bool {
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
        &["Effect", "ModifyTargetKeywordUntilEndOfTurn"],
    ) || effect.rest.is_some()
        || effect.fields.len() != 1
    {
        return false;
    }
    let field = &effect.fields[0];
    matches!(&field.member, syn::Member::Named(name) if name == "keyword")
        && matches!(&field.expr, Expr::Path(value) if value.qself.is_none() && path_is(&value.path, &["Keyword", keyword]))
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    HasteEffect,
    FirstStrikeEffect,
    HasteCost,
    FirstStrikeCost,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    guildmage: Option<ObjectId>,
    post_haste_keywords: Option<Vec<String>>,
    post_first_strike_keywords: Option<Vec<String>>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BOROS_GUILDMAGE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BOROS_GUILDMAGE_SEED must be an unsigned integer".to_owned())?;
        let preview = run_match_inner(seed, Mutation::None, true)?;
        validate_trace(&preview, seed)?;
        let full = run_match(seed, Mutation::None)?;
        validate_trace(&full, seed)?;
        return Ok((seed, full));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(preview) = run_match_inner(seed, Mutation::None, true)
            && validate_trace(&preview, seed).is_ok()
        {
            let captured = run_match(seed, Mutation::None)?;
            validate_trace(&captured, seed)?;
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no complete Boros Guildmage activated-combat trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(error) if error.contains("policy move rejected") => Ok(true),
        Err(error) => Err(format!(
            "{mutation:?} mutation run failed before the trace oracle: {error}"
        )),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    run_match_inner(seed, mutation, false)
}

fn run_match_inner(
    seed: u64,
    mutation: Mutation,
    stop_after_combat: bool,
) -> Result<CapturedMatch, String> {
    let definition = candidate_boros_guildmage::definition();
    let mut haste = candidate_boros_guildmage::grant_haste_ability();
    let mut first_strike = candidate_boros_guildmage::grant_first_strike_ability();
    match mutation {
        Mutation::None => {}
        Mutation::HasteEffect => {
            haste.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Vigilance,
            }]
        }
        Mutation::FirstStrikeEffect => {
            first_strike.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Vigilance,
            }]
        }
        Mutation::HasteCost => haste.mana_cost.generic = 1,
        Mutation::FirstStrikeCost => first_strike.mana_cost.generic = 1,
    }
    let mut game = new_rav_game(definition.clone(), haste, first_strike)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-ELVES-OF-DEEP-SHADOW".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("seat zero", &deck0), ("seat one", &deck1)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
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
        Box::new(GuildmagePolicy::new(PlayerId(0))),
        Box::new(ElfBlockPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut haste_snapshot = None;
    let mut first_strike_snapshot = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Boros Guildmage game hit a bounded development limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Boros Guildmage policy seat is missing")?;
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
        let source = game.event_log.iter().find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|card| card.id == CARD_ID) =>
            {
                Some(*card)
            }
            _ => None,
        });
        if let Some(source) = source {
            if haste_snapshot.is_none() && game.event_log.iter().any(|event| matches!(event,
                GameEvent::AbilityResolved { source: resolved, ability: "grant-haste", .. } if *resolved == source))
            {
                haste_snapshot = game.characteristics(source).ok().map(|chars| chars.keywords.iter().map(|keyword| format!("{keyword:?}")).collect());
            }
            if first_strike_snapshot.is_none() && game.event_log.iter().any(|event| matches!(event,
                GameEvent::AbilityResolved { source: resolved, ability: "grant-first-strike", .. } if *resolved == source))
            {
                first_strike_snapshot = game.characteristics(source).ok().map(|chars| chars.keywords.iter().map(|keyword| format!("{keyword:?}")).collect());
            }
        }
        if stop_after_combat {
            let source = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
            if source.is_none() && game.turn > 8 {
                break;
            }
            if source.is_some_and(|source| {
                game.event_log.iter().any(|event| matches!(event,
                GameEvent::StepBegan { turn, active_player: PlayerId(0), step: Step::EndOfCombat }
                    if *turn == game.turn
            )) && game.event_log.iter().any(|event| matches!(event,
                GameEvent::SpellCast { player: PlayerId(0), card } if *card == source
            ))
            }) {
                break;
            }
        }
    }
    let winner = game.winner().map(|player| player.0);
    let guildmage = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|card| card.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: if !game.is_game_over() {
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
        guildmage,
        post_haste_keywords: haste_snapshot,
        post_first_strike_keywords: first_strike_snapshot,
    })
}

fn new_rav_game(
    definition: CardDefinition,
    haste: ActivatedAbility,
    first_strike: ActivatedAbility,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Boros Guildmage missing from RAV definition registry")?;
    *registered = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| binding.card_definition != CARD_ID);
    abilities.push(ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: haste,
    });
    abilities.push(ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: first_strike,
    });
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

struct GuildmagePolicy {
    player: PlayerId,
    haste_activated: bool,
    first_strike_activated: bool,
}

impl GuildmagePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            haste_activated: false,
            first_strike_activated: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let red_lands = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Red)
            })
            .count();
        let white_lands = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            })
            .count();
        let preferred = if red_lands <= white_lands {
            Color::Red
        } else {
            Color::White
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

    fn make_mana(&self, view: &GameView, preferred: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&preferred)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: preferred,
        })
    }

    fn has_guildmage_in_hand(&self, view: &GameView) -> bool {
        view.hand
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
    }

    fn choose_cast_mana(&self, view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::Red) < 2 || view.mana_pool.amount(Color::White) < 2 {
            return None;
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![],
                hybrid: vec![Color::Red, Color::White],
            },
        })
    }
}

impl CodePolicy for GuildmagePolicy {
    fn id(&self) -> &'static str {
        "rav.boros_guildmage-activate-and-combat.v1"
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
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
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
        if let Some(action) = self.play_land(view) {
            return action;
        }

        let source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID));
        if let Some(source) = source {
            if !self.haste_activated {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.haste_activated = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: "grant-haste",
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(source.id)],
                        },
                    };
                }
                if let Some(action) = self.make_mana(view, Color::Red) {
                    return action;
                }
            } else if !self.first_strike_activated {
                if view.mana_pool.amount(Color::White) > 0 {
                    self.first_strike_activated = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: "grant-first-strike",
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(source.id)],
                        },
                    };
                }
                if let Some(action) = self.make_mana(view, Color::White) {
                    return action;
                }
            }
        } else if self.has_guildmage_in_hand(view) {
            if let Some(action) = self.choose_cast_mana(view) {
                return action;
            }
            let red = view.mana_pool.amount(Color::Red);
            let white = view.mana_pool.amount(Color::White);
            let preferred = if red < 2 && (red <= white || white >= 2) {
                Color::Red
            } else {
                Color::White
            };
            if let Some(action) = self.make_mana(view, preferred) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

struct ElfBlockPolicy {
    player: PlayerId,
}

impl ElfBlockPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for ElfBlockPolicy {
    fn id(&self) -> &'static str {
        "rav.boros_guildmage-play-elf-and-block.v1"
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
            let assignment = view
                .combat_attackers
                .iter()
                .find(|attacker| attacker.definition == Some(CARD_ID))
                .and_then(|attacker| {
                    view.own_battlefield
                        .iter()
                        .find(|blocker| {
                            blocker.definition == Some("RAV-ELVES-OF-DEEP-SHADOW")
                                && blocker.can_block
                        })
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
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if view.mana_pool.amount(Color::Green) > 0 {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-ELVES-OF-DEEP-SHADOW"))
            {
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![],
                        hybrid: vec![],
                    },
                };
            }
        }
        if let Some(land) = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&Color::Green)
        }) {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Green,
            };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let completed = captured.terminal != "InProgress";
    if completed && (captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0)) {
        return Err("Boros Guildmage game did not reach a clean seat-zero terminal win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Boros Guildmage trace contains rejected moves".to_owned());
    }
    let guildmage = captured
        .guildmage
        .ok_or("Boros Guildmage was not cast from the shuffled library")?;
    let post_haste = captured
        .post_haste_keywords
        .as_ref()
        .ok_or("Haste activation did not resolve")?;
    let post_first_strike = captured
        .post_first_strike_keywords
        .as_ref()
        .ok_or("First Strike activation did not resolve")?;
    if !post_haste.iter().any(|keyword| keyword == "Haste")
        || !post_first_strike.iter().any(|keyword| keyword == "Haste")
        || !post_first_strike
            .iter()
            .any(|keyword| keyword == "FirstStrike")
    {
        return Err("activated target did not gain live Haste and First Strike".to_owned());
    }
    let blocker = captured
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if assignments
                .iter()
                .any(|(attacker, _)| *attacker == guildmage) =>
            {
                assignments
                    .iter()
                    .find(|(attacker, _)| *attacker == guildmage)
                    .map(|(_, blocker)| *blocker)
            }
            _ => None,
        })
        .ok_or("Haste/First Strike target was not blocked by a real opposing creature")?;
    let projected = project_events(&captured.events);
    let cast_turn = projected
        .iter()
        .find(|event| {
            event.kind == "SpellCast"
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == guildmage.0)
        })
        .map(|event| event.turn)
        .ok_or("Boros Guildmage SpellCast event is missing")?;
    let attack = projected
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared"
                && event.turn == cast_turn
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == guildmage.0)
        })
        .ok_or("Haste-granted Guildmage did not attack on the turn it entered")?;
    let block = projected
        .iter()
        .find(|event| {
            event.kind == "BlockersDeclared"
                && event.turn == cast_turn
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == guildmage.0)
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == blocker.0)
        })
        .ok_or("real blocker did not block the entering Haste target")?;
    let relevant = relevant_events(&projected, guildmage, blocker, cast_turn);
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    let mut expected = vec![
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityManaPaid",
        "AbilityActivated",
        "AbilityResolved",
        "AbilityManaPaid",
        "AbilityActivated",
        "AbilityResolved",
        "AttackersDeclared",
        "BlockersDeclared",
        "StepBegan",
        "DamageDealtToPermanent",
        "StateBasedAction",
        "CardMoved",
        "StepBegan",
    ];
    if completed {
        expected.push("GameEnded");
    }
    if kinds != expected {
        return Err(format!("Boros Guildmage event order differs: {kinds:?}"));
    }
    let sequences: Vec<_> = relevant.iter().map(|event| event.sequence).collect();
    let has_red = relevant[4].canonical.contains("ability: \"grant-haste\"")
        && relevant[4].canonical.contains("colored: [Red]");
    let has_white = relevant[7]
        .canonical
        .contains("ability: \"grant-first-strike\"")
        && relevant[7].canonical.contains("colored: [White]");
    if !has_red
        || !has_white
        || !relevant[5].canonical.contains("grant-haste")
        || !relevant[6].canonical.contains("grant-haste")
        || !relevant[8].canonical.contains("grant-first-strike")
        || !relevant[9].canonical.contains("grant-first-strike")
        || relevant[0].facts.mana_colors.len() != 2
        || !relevant[0].facts.mana_colors.contains(&"Red".to_owned())
        || !relevant[0].facts.mana_colors.contains(&"White".to_owned())
        || relevant[1].facts.objects.first().map(|object| object.0) != Some(guildmage.0)
        || relevant[3].facts.zone.as_deref() != Some("Battlefield")
        || !relevant[10]
            .facts
            .objects
            .iter()
            .any(|object| object.0 == guildmage.0)
        || !relevant[11]
            .facts
            .objects
            .iter()
            .any(|object| object.0 == blocker.0)
        || relevant[12].facts.step.as_deref() != Some("FirstStrikeCombatDamage")
        || relevant[13].facts.objects.as_slice()
            != [
                cardbench_magic_protocol::ObjectRef(guildmage.0),
                cardbench_magic_protocol::ObjectRef(blocker.0),
            ]
        || relevant[13].facts.amount != Some(2)
        || relevant[14].facts.objects.first().map(|object| object.0) != Some(blocker.0)
        || relevant[14].facts.reason.as_deref() != Some("creature has lethal damage")
        || relevant[15].facts.objects.first().map(|object| object.0) != Some(blocker.0)
        || relevant[15].facts.zone.as_deref() != Some("Graveyard")
        || relevant[16].facts.step.as_deref() != Some("CombatDamage")
        || completed
            && relevant.last().is_none_or(|event| {
                event.kind != "GameEnded" || event.facts.seats.first().map(|seat| seat.0) != Some(0)
            })
        || !sequences.windows(2).all(|pair| pair[0] < pair[1])
        || attack.turn != cast_turn
        || block.turn != cast_turn
    {
        return Err("Boros Guildmage payment, target, or damage-step facts differ".to_owned());
    }
    if captured.events.iter().any(|event| matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == blocker && *permanent == guildmage))
    {
        return Err("First Strike blocker dealt damage before dying".to_owned());
    }
    if captured.events.iter().any(|event| matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 } if *source == guildmage && *permanent == blocker))
        == false
    {
        return Err("First Strike combat damage did not destroy the real blocker".to_owned());
    }
    Ok(json!({
        "id": "rav-boros-guildmage-dual-keyword-activations-combat",
        "seed": seed,
        "decks": ["custom-rav-boros-guildmage", "custom-rav-elf-blocker"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "boros_guildmage_object": guildmage.0,
        "target_object": guildmage.0,
        "blocker_object": blocker.0,
        "same_turn_haste_attack": true,
        "post_haste_keywords": post_haste,
        "post_first_strike_keywords": post_first_strike,
        "blocker_in_graveyard_after_combat": true,
        "blocker_dealt_no_damage": true,
        "ordered_event_sequence": sequences,
        "events": relevant,
        "passed": true
    }))
}

fn relevant_events(
    events: &[ProjectedEvent],
    guildmage: ObjectId,
    blocker: ObjectId,
    cast_turn: u32,
) -> Vec<ProjectedEvent> {
    events
        .iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid" | "SpellCast" | "SpellResolved" => event
                .facts
                .objects
                .iter()
                .any(|object| object.0 == guildmage.0),
            "CardMoved" => {
                event.turn == cast_turn
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == guildmage.0)
                    && event.facts.zone.as_deref() == Some("Battlefield")
                    || event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == blocker.0)
                        && event.facts.zone.as_deref() == Some("Graveyard")
            }
            "AbilityManaPaid" | "AbilityActivated" | "AbilityResolved" => {
                event
                    .canonical
                    .contains(&format!("source: ObjectId({})", guildmage.0))
                    && (event.canonical.contains("grant-haste")
                        || event.canonical.contains("grant-first-strike"))
            }
            "AttackersDeclared" => {
                event.turn == cast_turn
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == guildmage.0)
            }
            "BlockersDeclared" => {
                event.turn == cast_turn
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == guildmage.0)
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == blocker.0)
            }
            "StepBegan" => {
                event.turn == cast_turn
                    && matches!(
                        event.facts.step.as_deref(),
                        Some("FirstStrikeCombatDamage" | "CombatDamage")
                    )
            }
            "DamageDealtToPermanent" => {
                event.turn == cast_turn
                    && event.facts.objects.len() == 2
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == guildmage.0)
                    && event
                        .facts
                        .objects
                        .iter()
                        .any(|object| object.0 == blocker.0)
            }
            "StateBasedAction" => {
                event.turn == cast_turn
                    && event.facts.objects.first().map(|object| object.0) == Some(blocker.0)
            }
            "GameEnded" => true,
            _ => false,
        })
        .cloned()
        .collect()
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 242;
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
