//! Full-game RAV Ordruun Commando cast, white activation, and combat-prevention trace.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, PathArguments, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ORDRUUN-COMMANDO";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const ABILITY_ID: &str = "prevent-one-damage-to-self";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/ordruun_commando.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/ordruun_commando.rs"]
mod candidate_ordruun_commando;

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
            eprintln!("Ordruun Commando PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.commando != replay.commando
        || captured.elf != replay.elf
    {
        return Err("Ordruun Commando shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "shield_amount_mutation_detected_in_game": mutation_detected(seed, Mutation::NoShield)?,
        "activation_color_mutation_detected_in_game": mutation_detected(seed, Mutation::RedActivation)?,
        "activation_tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::TapSource)?,
        "red_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "shield_amount_mutation_detected_in_game",
        "activation_color_mutation_detected_in_game",
        "activation_tap_cost_mutation_detected_in_game",
        "red_cast_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Ordruun Commando mutation escaped the trace oracle: {key}"
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
        "card": {"collector_number": 137, "definition_id": CARD_ID, "name": "Ordruun Commando"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities, targets, and costs",
                "608.2 Resolving an activated ability",
                "510.1-510.2 Combat damage assignment and resolution",
                "615.3-615.7 Prevention effects and one-point damage shields",
                "611.2a End-of-turn continuous-effect duration"
            ],
            "source_contract": "RAV Ordruun Commando #137, {3}{R} 4/1 creature, and {W}: prevent the next 1 damage that would be dealt to Ordruun Commando this turn",
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
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    NoShield,
    RedActivation,
    TapSource,
    WrongCastCost,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_ordruun_commando::definition();
    let mut ability = candidate_ordruun_commando::prevent_one_damage_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoShield => {
            ability.effects = vec![Effect::AddSourceDamageShieldUntilEndOfTurn { amount: 0 }];
        }
        Mutation::RedActivation => {
            ability.mana_cost = cardbench_magic_engine::ManaCost::with_colors(0, [Color::Red]);
        }
        Mutation::TapSource => ability.tap_cost = true,
        Mutation::WrongCastCost => definition.mana_cost = cardbench_magic_engine::ManaCost::new(4),
    }
    (definition, ability)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|registered| registered.id == CARD_ID)
        .ok_or("Ordruun Commando definition is missing from the RAV registry")?;
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ordruun Commando activated ability is missing from the RAV registry")?;
    if registered_definition != definition || registered_ability.ability != ability {
        return Err("Ordruun Commando definition or activated ability is not delegated to its candidate module".to_owned());
    }
    if definition.id != CARD_ID
        || definition.name != "Ordruun Commando"
        || definition.set_code != "RAV"
        || definition.mana_cost != cardbench_magic_engine::ManaCost::with_colors(3, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(4)
        || definition.toughness != Some(1)
        || definition.is_basic_land
        || !definition.effects.is_empty()
        || !definition.keywords.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != cardbench_magic_engine::ManaCost::with_colors(0, [Color::White])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects != [Effect::AddSourceDamageShieldUntilEndOfTurn { amount: 1 }]
    {
        return Err(
            "Ordruun Commando identity or shield ability differs from its feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Ordruun Commando source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Ordruun Commando source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Ordruun Commando has unsupported module attributes".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual = BTreeSet::new();
    let mut definition = None;
    let mut ability = None;
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) =>
            {
                actual.extend(
                    flatten_use(&use_item.tree)
                        .ok_or("Ordruun Commando has an unsupported import")?,
                );
            }
            Item::Fn(function) if function.sig.ident == "definition" && definition.is_none() => {
                definition = Some(function)
            }
            Item::Fn(function)
                if function.sig.ident == "prevent_one_damage_ability" && ability.is_none() =>
            {
                ability = Some(function)
            }
            _ => {
                return Err(
                    "Ordruun Commando module contains unsupported executable items".to_owned(),
                );
            }
        }
    }
    if actual != expected_imports || file.items.len() != 4 {
        return Err(
            "Ordruun Commando imports or literal module structure differ from the source contract"
                .to_owned(),
        );
    }
    let def = direct_struct(
        definition.ok_or("Ordruun Commando definition() is missing")?,
        "definition",
        "CardDefinition",
    )?;
    let def_fields = literal_fields(def)?;
    if def_fields.len() != 13
        || !str_field(def_fields.get("id"), CARD_ID)
        || !str_field(def_fields.get("name"), "Ordruun Commando")
        || !str_field(def_fields.get("set_code"), "RAV")
        || !mana_cost(def_fields.get("mana_cost"), 3, &["Red"])
        || !set_paths(def_fields.get("colors"), &[&["Color", "Red"]])
        || !empty_set(def_fields.get("mana_colors"))
        || !set_paths(def_fields.get("card_types"), &[&["CardType", "Creature"]])
        || !bool_field(def_fields.get("is_basic_land"), false)
        || !rules_field(
            def_fields.get("supported_rules"),
            &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-prevent-one-damage-to-self",
            ],
        )
        || !some_number(def_fields.get("power"), 4)
        || !some_number(def_fields.get("toughness"), 1)
        || !empty_vec(def_fields.get("keywords"))
        || !empty_vec(def_fields.get("effects"))
    {
        return Err(
            "Ordruun Commando definition is outside its literal source contract".to_owned(),
        );
    }
    let ability = direct_struct(
        ability.ok_or("Ordruun Commando prevention ability is missing")?,
        "prevent_one_damage_ability",
        "ActivatedAbility",
    )?;
    let fields = literal_fields(ability)?;
    if fields.len() != 11
        || !str_field(fields.get("id"), ABILITY_ID)
        || !mana_cost(fields.get("mana_cost"), 0, &["White"])
        || !bool_field(fields.get("tap_cost"), false)
        || !bool_field(fields.get("sorcery_speed"), false)
        || !number_field(fields.get("additional_tap_creatures"), 0)
        || !bool_field(fields.get("sacrifice_source"), false)
        || !number_field(fields.get("sacrifice_creatures"), 0)
        || !number_field(fields.get("sacrifice_lands"), 0)
        || !number_field(fields.get("discard_cards"), 0)
        || !empty_vec(fields.get("targets"))
        || !shield_effect(fields.get("effects"), 1)
    {
        return Err(
            "Ordruun Commando activated ability is outside its literal source contract".to_owned(),
        );
    }
    Ok(())
}

fn is_doc_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("doc") && matches!(attr.meta, syn::Meta::NameValue(_))
}

fn flatten_use(tree: &UseTree) -> Option<Vec<Vec<String>>> {
    fn walk(prefix: Vec<String>, tree: &UseTree, out: &mut Vec<Vec<String>>) -> Option<()> {
        match tree {
            UseTree::Path(path) => {
                let mut next = prefix;
                next.push(path.ident.to_string());
                walk(next, &path.tree, out)
            }
            UseTree::Name(name) => {
                let mut next = prefix;
                next.push(name.ident.to_string());
                out.push(next);
                Some(())
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    walk(prefix.clone(), item, out)?;
                }
                Some(())
            }
            UseTree::Rename(_) | UseTree::Glob(_) => None,
        }
    }
    let mut out = Vec::new();
    walk(Vec::new(), tree, &mut out)?;
    Some(out)
}

fn direct_struct<'a>(
    function: &'a syn::ItemFn,
    name: &str,
    result: &str,
) -> Result<&'a syn::ExprStruct, String> {
    let allowed_attrs = function.attrs.iter().all(|attr| {
        is_doc_attribute(attr)
            || attr.path().is_ident("must_use") && matches!(attr.meta, syn::Meta::Path(_))
    });
    let crate_vis =
        matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
    if !allowed_attrs
        || !crate_vis
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || function.block.stmts.len() != 1
        || !matches!(&function.sig.output,syn::ReturnType::Type(_,ty) if path_is_type(ty,result))
    {
        return Err(format!(
            "Ordruun Commando {name} has an unsupported signature"
        ));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Ordruun Commando {name} must return one direct literal"
        ));
    };
    if !path_is(&value.path, &[result]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err(format!("Ordruun Commando {name} literal type differs"));
    }
    Ok(value)
}

fn path_is_type(ty: &syn::Type, last: &str) -> bool {
    matches!(ty,syn::Type::Path(ty) if ty.qself.is_none() && ty.path.segments.len()==1 && ty.path.segments[0].ident==last && matches!(ty.path.segments[0].arguments,PathArguments::None))
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(part, want)| {
            part.ident == *want && matches!(part.arguments, PathArguments::None)
        })
}
fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut out = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Ordruun Commando struct fields must be named".to_owned());
        };
        if field.colon_token.is_none() || out.insert(name.to_string(), &field.expr).is_some() {
            return Err("Ordruun Commando struct has shorthand or duplicate fields".to_owned());
        }
    }
    Ok(out)
}
fn str_field(expr: Option<&&Expr>, expected: &str) -> bool {
    matches!(expr,Some(Expr::Lit(lit)) if lit.attrs.is_empty() && matches!(&lit.lit,Lit::Str(value) if value.value()==expected))
}
fn bool_field(expr: Option<&&Expr>, expected: bool) -> bool {
    matches!(expr,Some(Expr::Lit(lit)) if lit.attrs.is_empty() && matches!(&lit.lit,Lit::Bool(value) if value.value==expected))
}
fn number_field(expr: Option<&&Expr>, expected: u64) -> bool {
    matches!(expr,Some(Expr::Lit(lit)) if lit.attrs.is_empty() && matches!(&lit.lit,Lit::Int(value) if value.base10_parse::<u64>().ok()==Some(expected)))
}
fn call_args<'a>(expr: Option<&&'a Expr>, path: &[&str]) -> Option<Vec<&'a Expr>> {
    match expr? {
        Expr::Call(call)
            if call.attrs.is_empty()
                && matches!(&*call.func,Expr::Path(p) if p.qself.is_none() && path_is(&p.path,path)) =>
        {
            Some(call.args.iter().collect())
        }
        _ => None,
    }
}
fn arr(expr: &Expr) -> Option<Vec<&Expr>> {
    match expr {
        Expr::Array(a) if a.attrs.is_empty() => Some(a.elems.iter().collect()),
        _ => None,
    }
}
fn expr_path(expr: &Expr, path: &[&str]) -> bool {
    matches!(expr,Expr::Path(p) if p.attrs.is_empty() && p.qself.is_none() && path_is(&p.path,path))
}
fn mana_cost(expr: Option<&&Expr>, generic: u64, colors: &[&str]) -> bool {
    let Some(args) = call_args(expr, &["ManaCost", "with_colors"]) else {
        return false;
    };
    args.len() == 2
        && number_expr(args[0], generic)
        && arr(args[1]).is_some_and(|items| {
            items.len() == colors.len()
                && items
                    .iter()
                    .zip(colors)
                    .all(|(item, color)| expr_path(item, &["Color", color]))
        })
}
fn number_expr(expr: &Expr, expected: u64) -> bool {
    matches!(expr,Expr::Lit(lit) if lit.attrs.is_empty() && matches!(&lit.lit,Lit::Int(value) if value.base10_parse::<u64>().ok()==Some(expected)))
}
fn empty_set(expr: Option<&&Expr>) -> bool {
    call_args(expr, &["BTreeSet", "new"]).is_some_and(|items| items.is_empty())
}
fn set_paths(expr: Option<&&Expr>, expected: &[&[&str]]) -> bool {
    call_args(expr, &["BTreeSet", "from"]).is_some_and(|args| {
        args.len() == 1
            && arr(args[0]).is_some_and(|items| {
                items.len() == expected.len()
                    && items.iter().zip(expected).all(|(a, b)| expr_path(a, b))
            })
    })
}
fn rules_field(expr: Option<&&Expr>, expected: &[&str]) -> bool {
    matches!(expr,Some(Expr::Reference(r)) if r.mutability.is_none() && arr(&r.expr).is_some_and(|items|items.len()==expected.len() && items.iter().zip(expected).all(|(item,text)|matches!(item,Expr::Lit(lit) if matches!(&lit.lit,Lit::Str(value) if value.value()==*text)))))
}
fn some_number(expr: Option<&&Expr>, expected: u64) -> bool {
    call_args(expr, &["Some"]).is_some_and(|args| args.len() == 1 && number_expr(args[0], expected))
}
fn empty_vec(expr: Option<&&Expr>) -> bool {
    matches!(expr,Some(Expr::Macro(mac)) if mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty())
}
fn macro_items(mac: &syn::Macro) -> Option<Vec<Expr>> {
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(mac.tokens.clone())
        .ok()
        .map(IntoIterator::into_iter)
        .map(Iterator::collect)
}
fn shield_effect(expr: Option<&&Expr>, amount: u64) -> bool {
    let Some(Expr::Macro(mac)) = expr else {
        return false;
    };
    if !mac.mac.path.is_ident("vec") {
        return false;
    }
    let Some(items) = macro_items(&mac.mac) else {
        return false;
    };
    let [Expr::Struct(effect)] = items.as_slice() else {
        return false;
    };
    if !path_is(
        &effect.path,
        &["Effect", "AddSourceDamageShieldUntilEndOfTurn"],
    ) || effect.rest.is_some()
    {
        return false;
    }
    let Ok(fields) = literal_fields(effect) else {
        return false;
    };
    fields.len() == 1 && number_field(fields.get("amount"), amount)
}

fn find_trace() -> Result<(u64, Captured), String> {
    let requested = std::env::var("CARDBENCH_ORDRUUN_COMMANDO_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    if let Some(seed) = requested {
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no Ordruun Commando full-game prevention trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    commando: Option<ObjectId>,
    elf: Option<ObjectId>,
    cast_cost: cardbench_magic_engine::ManaCost,
    cast_payment: Vec<Color>,
    ability_payment: Option<cardbench_magic_engine::ManaCost>,
    cast_turn: Option<u32>,
    activation_turn: Option<u32>,
    activation_source: Option<ObjectId>,
    source_tapped_after_activation: bool,
    shield_after_resolution: Option<i32>,
    source_zone: Option<Zone>,
    target_zone: Option<Zone>,
    source_power_toughness: Option<(i32, i32)>,
    player_life: [i64; 2],
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let (mut game, _definitions) = new_game(definition, ability)?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 27,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 29,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ELF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if [p0.mainboard.as_slice(), p1.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| entry.count as usize)
                .sum::<usize>()
                != 60
        })
    {
        return Err("Ordruun Commando decks must be shuffled legal 60-card lists".to_owned());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(OrdruunPolicy::new(PlayerId(0), &cast_cost)),
        Box::new(OrdruunPolicy::new(
            PlayerId(1),
            &cardbench_magic_engine::ManaCost::with_colors(0, [Color::Green]),
        )),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut activation_turn = None;
    let mut activation_source = None;
    let mut source_tapped_after_activation = false;
    let mut shield_after_resolution = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Ordruun Commando match exceeded its bounded move count".to_owned());
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
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some(activation.source)
            }
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Ordruun Commando policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Ordruun Commando invariant failed after move: {error}"))?;
        if let Some(source) = activation {
            if activation_turn.is_none() {
                source_tapped_after_activation =
                    game.object(source).is_ok_and(|object| object.tapped);
                activation_source = Some(source);
                activation_turn = Some(view.turn);
            }
        }
        if shield_after_resolution.is_none()
            && let Some(source) = activation_source
            && game.event_log.iter().any(|event| matches!(event,
                GameEvent::AbilityResolved { source: resolved_source, ability: ABILITY_ID, .. } if *resolved_source == source))
        {
            shield_after_resolution = game.object(source).ok().map(|object| object.damage_shield);
        }
    }
    let events = game.event_log.clone();
    let commando = events.iter().find_map(|event| match event {
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
    let elf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == ELF_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let cast_payment = commando
        .and_then(|object| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == object => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let ability_payment = activation_source.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source: paid_source,
                ability: ABILITY_ID,
                mana_cost,
            } if *paid_source == source => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let source_zone = commando.and_then(|object| game.zone_of(object));
    let target_zone = elf.and_then(|object| {
        events.iter().find_map(|event| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if *card == object => Some(Zone::Graveyard),
            _ => None,
        })
    });
    let source_power_toughness = commando.and_then(|object| {
        let characteristics = game.characteristics(object).ok()?;
        Some((characteristics.power?, characteristics.toughness?))
    });
    let player_life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        commando,
        elf,
        cast_cost,
        cast_payment,
        ability_payment,
        cast_turn: commando.and_then(|object| first_turn_for_spell(&game.event_log, object)),
        activation_turn,
        activation_source,
        source_tapped_after_activation,
        shield_after_resolution,
        source_zone,
        target_zone,
        source_power_toughness,
        player_life,
    })
}

fn first_turn_for_spell(events: &[GameEvent], card: ObjectId) -> Option<u32> {
    let cast_index = events.iter().position(
        |event| matches!(event, GameEvent::SpellCast { card: cast, .. } if *cast == card),
    )?;
    events[..cast_index]
        .iter()
        .rev()
        .find_map(|event| match event {
            GameEvent::StepBegan {
                turn,
                step: Step::PrecombatMain,
                ..
            } => Some(*turn),
            _ => None,
        })
}

fn new_game(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<(Game, Vec<CardDefinition>), String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Ordruun Commando definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ordruun Commando activated ability binding missing from registry")?;
    binding.ability = ability;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions.clone(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|e| e.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|e| e.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    Ok((game, definitions))
}

struct OrdruunPolicy {
    player: PlayerId,
    cast_cost: cardbench_magic_engine::ManaCost,
    cast_offered: bool,
    elf_cast_offered: bool,
    ability_offered: bool,
}

impl OrdruunPolicy {
    fn new(player: PlayerId, cost: &cardbench_magic_engine::ManaCost) -> Self {
        Self {
            player,
            cast_cost: cost.clone(),
            cast_offered: false,
            elf_cast_offered: false,
            ability_offered: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let land = if self.player == PlayerId(0) {
            let mountain_count = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Red)
                })
                .count();
            let plains_count = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::White)
                })
                .count();
            let preferred = if mountain_count <= plains_count {
                Color::Red
            } else {
                Color::White
            };
            view.hand
                .iter()
                .find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&preferred)
                })
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })?
        } else {
            view.hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))?
        };
        Some(PolicyAction::PlayLand { card: land.id })
    }

    fn tap_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment_selection(
        &self,
        view: &GameView,
        cost: &cardbench_magic_engine::ManaCost,
    ) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = available.get_mut(color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        if !cost.hybrid.is_empty() {
            return None;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, count)| **count > 0)
                .max_by_key(|(_, count)| **count)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn cast_action(
        &mut self,
        view: &GameView,
        card_id: &str,
        cost: &cardbench_magic_engine::ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = self.payment_selection(view, cost) {
            if card_id == CARD_ID {
                self.cast_offered = true;
            } else {
                self.elf_cast_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing_colored = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let total_required =
            u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
        if let Some(color) = missing_colored.or_else(|| {
            (u16::from(view.mana_pool.total()) < total_required)
                .then_some(cost.colored.first().copied().unwrap_or(Color::Red))
        }) {
            return self.tap_land(view, color);
        }
        None
    }

    fn activate_prevention(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.ability_offered
            || view.active_player == self.player
            || view.step != Step::DeclareAttackers
            || !view.attackers_declared
            || view.stack_depth != 0
        {
            return None;
        }
        if !view
            .combat_attackers
            .iter()
            .any(|attacker| attacker.definition == Some(ELF_ID))
        {
            return None;
        }
        let source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::White) == 0 {
            return self.tap_land(view, Color::White);
        }
        self.ability_offered = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: source.id,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }
}

impl CodePolicy for OrdruunPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.ordruun-commando-shield-and-combat.v1"
        } else {
            "rav.ordruun-commando-elf-pressure.v1"
        }
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
                    .filter(|card| {
                        card.can_attack
                            && (self.player == PlayerId(0) || card.definition == Some(ELF_ID))
                    })
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.player == PlayerId(0) {
                view.combat_attackers
                    .iter()
                    .find(|attacker| attacker.definition == Some(ELF_ID))
                    .and_then(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| {
                                blocker.definition == Some(CARD_ID) && blocker.can_block
                            })
                            .map(|blocker| cardbench_magic_engine::CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
                    .into_iter()
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.activate_prevention(view) {
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
        if self.player == PlayerId(1) {
            if !self.elf_cast_offered {
                if let Some(action) = self.cast_action(
                    view,
                    ELF_ID,
                    &cardbench_magic_engine::ManaCost::with_colors(0, [Color::Green]),
                ) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        let has_commando = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        if !has_commando && !self.cast_offered {
            let cast_cost = self.cast_cost.clone();
            if let Some(action) = self.cast_action(view, CARD_ID, &cast_cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Ordruun Commando match did not reach a clean seat-zero win".to_owned());
    }
    let commando = captured
        .commando
        .ok_or("seat zero did not cast Ordruun Commando")?;
    let elf = captured
        .elf
        .ok_or("seat one did not cast Elves of Deep Shadow")?;
    let expected_cast = cardbench_magic_engine::ManaCost::with_colors(3, [Color::Red]);
    let expected_ability = cardbench_magic_engine::ManaCost::with_colors(0, [Color::White]);
    if captured.cast_cost != expected_cast
        || captured.cast_payment.len() != 4
        || !captured.cast_payment.contains(&Color::Red)
    {
        return Err(format!(
            "Ordruun Commando did not pay {{3}}{{R}} using red mana among four total: {:?}",
            captured.cast_payment
        ));
    }
    if captured.ability_payment.as_ref() != Some(&expected_ability) {
        return Err("Ordruun Commando's activated ability did not pay {W}".to_owned());
    }
    if captured.activation_source != Some(commando) || captured.source_tapped_after_activation {
        return Err(
            "the source-bound prevention activation used the wrong source or tapped the creature"
                .to_owned(),
        );
    }
    if captured.shield_after_resolution != Some(1) {
        return Err(format!(
            "resolving {ABILITY_ID} did not put one damage shield on Commando: {:?}",
            captured.shield_after_resolution
        ));
    }
    if captured.source_zone != Some(Zone::Battlefield)
        || captured.target_zone != Some(Zone::Graveyard)
        || captured.source_power_toughness != Some((4, 1))
        || captured.player_life[1] > 0
    {
        return Err(format!(
            "prevention/combat state is wrong: source={:?}, target={:?}, pt={:?}, life={:?}",
            captured.source_zone,
            captured.target_zone,
            captured.source_power_toughness,
            captured.player_life
        ));
    }
    let events = &captured.events;
    let cast_payment = find_event(events, |event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == commando && colors.len() == 4 && colors.contains(&Color::Red))
    })
    .ok_or("Commando exact spell-payment receipt is missing")?;
    let cast = find_event_after(events, cast_payment, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == commando)
    })
    .ok_or("Commando SpellCast is missing")?;
    let resolved = find_event_after(events, cast, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == commando)
    })
    .ok_or("Commando SpellResolved is missing")?;
    let entered = find_event_after(events, resolved, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == commando)
    })
    .ok_or("Commando battlefield-entry receipt is missing")?;
    let elf_cast = find_event(events, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(1), card } if *card == elf)
    })
    .ok_or("opponent did not cast its live Elf target")?;
    let elf_entered = find_event_after(events, elf_cast, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == elf)
    })
    .ok_or("opposing Elf did not enter the battlefield")?;
    let elf_attack = events.iter().enumerate().find_map(|(index, event)| {
        (index > entered && index > elf_entered && matches!(event,
            GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&elf)))
            .then_some(index)
    }).ok_or("the live opposing Elf did not attack after Commando entered")?;
    let white_mana = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > elf_attack
                && matches!(
                    event,
                    GameEvent::ManaAbilityActivated {
                        player: PlayerId(0),
                        color: Color::White,
                        ..
                    }
                ))
            .then_some(index)
        })
        .ok_or("seat zero did not produce white mana during the opponent's attack turn")?;
    let ability_paid = find_event_after(events, white_mana, |event| {
        matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
            if *source == commando && mana_cost == &expected_ability)
    })
    .ok_or("the exact white activation payment receipt is missing")?;
    let activated = find_event_after(events, ability_paid, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. }
            if *source == commando))
        .ok_or("candidate-owned Ordruun ability activation receipt is missing")?;
    let ability_resolved = find_event_after(events, activated, |event| {
        matches!(event,
        GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == commando)
    })
    .ok_or("Ordruun prevention ability did not resolve")?;
    let block = find_event_after(events, ability_resolved, |event| {
        matches!(event,
        GameEvent::BlockersDeclared { player: PlayerId(0), assignments }
            if assignments.contains(&(elf, commando)))
    })
    .ok_or("Commando did not block the same attacking Elf after the shield resolved")?;
    let prevented = find_event_after(events, block, |event| {
        matches!(event,
        GameEvent::DamagePrevented { source, target: Target::Permanent(target), amount: 1 }
            if *source == elf && *target == commando)
    })
    .ok_or("the next one damage from the blocking interaction was not prevented")?;
    if events.iter().any(|event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == elf && *permanent == commando)
    }) {
        return Err(
            "prevented combat damage was also recorded as dealt damage to Commando".to_owned(),
        );
    }
    let commando_damage = find_event_after(events, prevented, |event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 }
            if *source == commando && *permanent == elf)
    })
    .ok_or("Commando did not deal its four combat damage to the blocked Elf")?;
    let lethal = find_event_after(events, commando_damage, |event| {
        matches!(event,
        GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == elf)
    })
    .ok_or("lethal combat damage did not trigger the Elf's state-based action")?;
    let elf_died = find_event_after(events, lethal, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == elf)
    })
    .ok_or("the defeated Elf did not move to its graveyard")?;
    let later_attack = find_event_after(events, elf_died, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&commando)))
        .ok_or("the surviving Commando did not attack later in this same game")?;
    let face_damage = find_event_after(events, later_attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == commando))
        .ok_or("Commando's later attack did not hit the opponent for four")?;
    let ended = find_event_after(events, face_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat zero's terminal game win is missing")?;
    if !(cast_payment < cast
        && cast < resolved
        && resolved < entered
        && elf_attack < white_mana
        && white_mana < ability_paid
        && ability_paid < activated
        && activated < ability_resolved
        && ability_resolved < block
        && block < prevented
        && prevented < commando_damage
        && commando_damage < lethal
        && lethal < elf_died
        && elf_died < later_attack
        && later_attack < face_damage
        && face_damage < ended)
    {
        return Err("Ordruun Commando's payment, activation, prevention, and combat receipts were out of order".to_owned());
    }
    let selected = BTreeSet::from([
        cast_payment,
        cast,
        resolved,
        entered,
        elf_cast,
        elf_entered,
        elf_attack,
        white_mana,
        ability_paid,
        activated,
        ability_resolved,
        block,
        prevented,
        commando_damage,
        lethal,
        elf_died,
        later_attack,
        face_damage,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    if !kinds.contains(&"AbilityManaPaid")
        || !kinds.contains(&"AbilityActivated")
        || !kinds.contains(&"AbilityResolved")
        || !kinds.contains(&"DamagePrevented")
        || !kinds.contains(&"BlockersDeclared")
        || !kinds.contains(&"GameEnded")
    {
        return Err(format!(
            "projected Ordruun trace omitted required receipts: {kinds:?}"
        ));
    }
    Ok(json!({
        "trace_id": "rav-ordruun-commando-white-shield-combat-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_ordruun_commando_mountain_plains", "rav_ordruun_elf_forest"],
        "policies": ["rav.ordruun-commando-shield-and-combat.v1", "rav.ordruun-commando-elf-pressure.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "commando": {"object": commando.0, "mana_cost": "{3}{R}", "mana_paid": captured.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "zone": "Battlefield", "power": 4, "toughness": 1, "cast_turn": captured.cast_turn},
        "attacker": {"object": elf.0, "definition": ELF_ID, "blocked_by": commando.0, "damage_prevented": 1, "zone_after_combat": "Graveyard"},
        "activation": {"ability": ABILITY_ID, "source": commando.0, "mana_cost": "{W}", "white_mana_paid": true, "source_tapped_after_activation": captured.source_tapped_after_activation, "shield_after_resolution": captured.shield_after_resolution, "activation_turn": captured.activation_turn, "resolved_before_block_and_damage": ability_resolved < block && block < prevented},
        "combat": {"blocked_elf_damage_was_prevented": true, "commando_survived": true, "commando_later_attacked": true, "later_player_damage": 4, "life_at_terminal": captured.player_life, "opponent_life_at_terminal": captured.player_life[1]},
        "events": projected,
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let status = if card.semantic_status.is_executable() {
        "executable_compatibility_slice"
    } else {
        "catalog_only"
    };
    let definition_id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":status,"trace_status":if definition_id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if definition_id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
