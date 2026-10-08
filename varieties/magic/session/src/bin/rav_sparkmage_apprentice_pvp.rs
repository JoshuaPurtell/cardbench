//! Full-game RAV Sparkmage Apprentice authority for its targeted ETB damage trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
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
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings, CardSemanticStatus,
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
};
use cardbench_magic_session::project_events;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SPARKMAGE-APPRENTICE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sparkmage_apprentice.rs";
const ELF: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const ETB_ABILITY: &str = "etb-deal-one";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sparkmage_apprentice.rs"]
mod candidate_sparkmage_apprentice;

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
            eprintln!("Sparkmage Apprentice authority failed: {error}");
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
    if captured.events != replay.events || captured.winner != replay.winner {
        return Err(
            "Sparkmage Apprentice shuffled full-game replay was not deterministic".to_owned(),
        );
    }
    let mutations = json!({
        "one_damage_mutation_detected_in_game": mutation_detected(seed, Mutation::DamageTwo, "Sparkmage Apprentice ETB trigger did not deal exactly one damage"),
        "etb_trigger_removal_detected_in_game": mutation_detected(seed, Mutation::TriggerRemoved, "Sparkmage Apprentice ETB target was not chosen and confirmed on the stack"),
        "red_mana_symbol_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost, "Sparkmage Apprentice mana payment did not include exactly one red mana symbol"),
        "sealed_mutation_authority": false
    });
    for key in [
        "one_damage_mutation_detected_in_game",
        "etb_trigger_removal_detected_in_game",
        "red_mana_symbol_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Sparkmage Apprentice gameplay mutation escaped the oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 144, "definition_id": CARD_ID, "name": "Sparkmage Apprentice"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["117 Timing and priority", "302.1 Creature permanents", "601.2 Casting spells", "603.2-603.3 Triggered abilities and target selection", "608.2 Resolving spells and abilities", "704 State-based actions"],
            "source_contract": "RAV Sparkmage Apprentice identity, red 1/1 creature characteristics, and mandatory enters-the-battlefield one damage to target player or creature",
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
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
    let definition = candidate_sparkmage_apprentice::definition();
    if definition.id != CARD_ID
        || definition.name != "Sparkmage Apprentice"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err(
            "Sparkmage Apprentice typed card definition differs from the feature contract"
                .to_owned(),
        );
    }
    let binding = candidate_sparkmage_apprentice::triggered_ability_binding();
    if binding.card_definition != CARD_ID
        || binding.ability.id != ETB_ABILITY
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.mana_cost != ManaCost::new(0)
        || binding.ability.optional
        || binding.ability.targets != [TargetRequirement::PlayerOrCreature]
        || binding.ability.effects
            != [Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::PlayerOrCreature,
            }]
    {
        return Err(
            "Sparkmage Apprentice ETB binding differs from the feature contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sparkmage Apprentice source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sparkmage Apprentice source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Sparkmage Apprentice source has unsupported module attributes".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
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
                    return Err("Sparkmage Apprentice source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) => {
                if functions
                    .insert(function.sig.ident.to_string(), function)
                    .is_some()
                {
                    return Err(
                        "Sparkmage Apprentice source has duplicate helper functions".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "Sparkmage Apprentice source contains executable top-level code".to_owned(),
                )
            }
        }
    }
    if imports != expected_imports
        || functions.len() != 2
        || !functions.contains_key("definition")
        || !functions.contains_key("triggered_ability_binding")
    {
        return Err(
            "Sparkmage Apprentice source exports only the contract definition and trigger"
                .to_owned(),
        );
    }
    let definition = direct_struct_function(functions["definition"], "CardDefinition")?;
    let definition_fields = literal_fields(definition)?;
    if definition_fields.len() != 13
        || !check_string(definition_fields["id"], CARD_ID)
        || !check_string(definition_fields["name"], "Sparkmage Apprentice")
        || !check_string(definition_fields["set_code"], "RAV")
        || !check_red_cost(definition_fields["mana_cost"])
        || !check_set(definition_fields["colors"], &[&["Color", "Red"]])
        || !check_empty_set(definition_fields["mana_colors"])
        || !check_set(
            definition_fields["card_types"],
            &[&["CardType", "Creature"]],
        )
        || !check_bool(definition_fields["is_basic_land"], false)
        || !check_rules(
            definition_fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "etb-targeted-damage",
            ],
        )
        || !check_some_int(definition_fields["power"], 1)
        || !check_some_int(definition_fields["toughness"], 1)
        || !check_empty_vec(definition_fields["keywords"])
        || !check_empty_vec(definition_fields["effects"])
    {
        return Err(
            "Sparkmage Apprentice definition is outside its literal source contract".to_owned(),
        );
    }
    let binding = direct_struct_function(
        functions["triggered_ability_binding"],
        "TriggeredAbilityBinding",
    )?;
    let binding_fields = literal_fields(binding)?;
    if binding_fields.len() != 2 || !check_string(binding_fields["card_definition"], CARD_ID) {
        return Err(
            "Sparkmage Apprentice trigger is outside its literal source contract".to_owned(),
        );
    }
    let ability = direct_struct_expr(binding_fields["ability"], "TriggeredAbility")?;
    let ability_fields = literal_fields(ability)?;
    if ability_fields.len() != 6
        || !check_string(ability_fields["id"], ETB_ABILITY)
        || !is_path(
            ability_fields["condition"],
            &["TriggerCondition", "EntersBattlefield"],
        )
        || !is_zero_mana_cost(ability_fields["mana_cost"])
        || !check_bool(ability_fields["optional"], false)
        || !check_targeted_requirement(ability_fields["targets"], "PlayerOrCreature")
        || !check_trigger_damage(ability_fields["effects"], 1, "PlayerOrCreature")
    {
        return Err(
            "Sparkmage Apprentice ETB binding is outside its literal source contract".to_owned(),
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
            || (attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_)))
    });
    if !attrs_ok
        || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
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
        return Err("Sparkmage Apprentice helper has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Sparkmage Apprentice helper must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Sparkmage Apprentice helper has the wrong literal type".to_owned());
    }
    Ok(value)
}

fn direct_struct_expr<'a>(expr: &'a Expr, type_name: &str) -> Result<&'a syn::ExprStruct, String> {
    let Expr::Struct(value) = expr else {
        return Err("Sparkmage Apprentice nested struct field must be literal".to_owned());
    };
    if !path_is(&value.path, &[type_name]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Sparkmage Apprentice nested struct has the wrong type".to_owned());
    }
    Ok(value)
}

fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("named literal fields required".to_owned());
        };
        if field.colon_token.is_none()
            || field.attrs.len() != 0
            || fields.insert(name.to_string(), &field.expr).is_some()
        {
            return Err("duplicate, attributed, or shorthand literal field".to_owned());
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
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| segment.ident == *expected && segment.arguments.is_empty())
}

fn path_is_type(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[expected]))
}

fn is_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}

fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Str(value) if value.value() == expected))
}

fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Bool(value) if value.value == expected))
}

fn check_int(expr: &Expr, expected: i64) -> bool {
    matches!(expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(value) if value.base10_parse::<i64>().ok() == Some(expected)))
}

fn check_some_int(expr: &Expr, expected: i64) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    matches!(&*call.func, Expr::Path(path) if path.path.is_ident("Some"))
        && call.args.len() == 1
        && check_int(&call.args[0], expected)
}

fn is_zero_mana_cost(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "new"))
        && call.args.len() == 1
        && check_int(&call.args[0], 0)
}

fn check_red_cost(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    if !matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "with_colors"))
        || call.args.len() != 2
        || !check_int(&call.args[0], 1)
    {
        return false;
    }
    let Expr::Array(colors) = &call.args[1] else {
        return false;
    };
    colors.elems.len() == 1 && is_path(&colors.elems[0], &["Color", "Red"])
}

fn check_empty_set(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "new"))
        && call.args.is_empty()
}

fn check_set(expr: &Expr, expected: &[&[&str]]) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    if !matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "from"))
        || call.args.len() != 1
    {
        return false;
    }
    let Expr::Array(values) = &call.args[0] else {
        return false;
    };
    values.elems.len() == expected.len()
        && values
            .elems
            .iter()
            .zip(expected)
            .all(|(value, path)| is_path(value, path))
}

fn check_rules(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Reference(reference) = expr else {
        return false;
    };
    let Expr::Array(values) = &*reference.expr else {
        return false;
    };
    values.elems.len() == expected.len()
        && values
            .elems
            .iter()
            .zip(expected)
            .all(|(value, expected)| check_string(value, expected))
}

fn vec_items(expr: &Expr) -> Option<Vec<Expr>> {
    let Expr::Macro(value) = expr else {
        return None;
    };
    if !value.mac.path.is_ident("vec") || !value.attrs.is_empty() {
        return None;
    }
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(value.mac.tokens.clone())
        .ok()
        .map(IntoIterator::into_iter)
        .map(Iterator::collect)
}

fn check_empty_vec(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|values| values.is_empty())
}

fn check_targeted_requirement(expr: &Expr, requirement: &str) -> bool {
    vec_items(expr).is_some_and(|values| {
        values.len() == 1 && is_path(&values[0], &["TargetRequirement", requirement])
    })
}

fn check_trigger_damage(expr: &Expr, amount: i64, requirement: &str) -> bool {
    let Some(values) = vec_items(expr) else {
        return false;
    };
    if values.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &values[0] else {
        return false;
    };
    if !path_is(&effect.path, &["Effect", "DealDamage"])
        || effect.rest.is_some()
        || effect.fields.len() != 2
    {
        return false;
    }
    let fields: BTreeMap<_, _> = effect
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(name) if field.attrs.is_empty() => {
                Some((name.to_string(), &field.expr))
            }
            _ => None,
        })
        .collect();
    fields.len() == 2
        && fields
            .get("amount")
            .is_some_and(|field| check_int(field, amount))
        && fields
            .get("target")
            .is_some_and(|field| is_path(field, &["TargetRequirement", requirement]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    DamageTwo,
    TriggerRemoved,
    GenericCost,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    apprentice: ObjectId,
    target_source: Option<ObjectId>,
    target: Option<ObjectId>,
    apprentice_zone: Option<Zone>,
    mana_paid: Option<Vec<Color>>,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SPARKMAGE_APPRENTICE_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_SPARKMAGE_APPRENTICE_SEED must be an unsigned integer".to_owned()
        })?;
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
        "no Sparkmage Apprentice PvP trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_sparkmage_apprentice::definition();
    if mutation == Mutation::GenericCost {
        definition.mana_cost = ManaCost::new(2);
    }
    let mut game = new_rav_game(definition, mutation)?;
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
                card: "RAV-FOREST".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ELF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let total0: usize = deck0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    let total1: usize = deck1
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if total0 != 60 || total1 != 60 {
        return Err(
            "Sparkmage Apprentice fixtures must be ordinary shuffled 60-card decks".to_owned(),
        );
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 8_000,
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
        Box::new(SparkmagePolicy::new(
            PlayerId(0),
            mutation == Mutation::GenericCost,
        )),
        Box::new(SparkmagePolicy::new(PlayerId(1), false)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut selected_source = None;
    let mut selected_target = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!("Sparkmage Apprentice match hit a bounded game limit: turn={}, step={:?}, life=[{}, {}]", game.turn, game.step, game.players[0].life, game.players[1].life));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Sparkmage Apprentice policy seat missing")?;
        let action = if view
            .triggered_ability_target_choice
            .as_ref()
            .is_some_and(|choice| choice.ability == ETB_ABILITY)
        {
            let choice = view
                .triggered_ability_target_choice
                .as_ref()
                .expect("matching Sparkmage Apprentice target decision");
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(ELF))
                .map(|card| Target::Permanent(card.id))
                .filter(|target| {
                    choice
                        .target_options
                        .first()
                        .is_some_and(|options| options.contains(target))
                })
                .ok_or("Sparkmage Apprentice ETB target decision has no legal Elf choice")?;
            PolicyAction::ChooseTriggeredAbilityTargets {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                targets: vec![target],
            }
        } else if let Some(action) = policy.propose_pending_decision(&view) {
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
        let chosen = match &action {
            PolicyAction::ChooseTriggeredAbilityTargets {
                source,
                ability,
                targets,
                ..
            } if *ability == ETB_ABILITY && targets.len() == 1 => Some((*source, targets[0])),
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Sparkmage Apprentice policy move rejected at turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if let Some((source, target)) = chosen {
            let stack = game
                .stack
                .last()
                .ok_or("Sparkmage Apprentice ETB trigger did not enter the stack")?;
            if stack.card != source
                || stack.ability_id != Some(ETB_ABILITY)
                || stack.targets != [target]
                || game.zone_of(source) != Some(Zone::Battlefield)
            {
                return Err(
                    "Sparkmage Apprentice ETB target was not confirmed on its live stack object"
                        .to_owned(),
                );
            }
            selected_target = match target {
                Target::Permanent(permanent)
                    if game
                        .card_definition(permanent)
                        .is_ok_and(|definition| definition.id == ELF) =>
                {
                    Some(permanent)
                }
                _ => None,
            };
            if selected_target.is_some() {
                selected_source = Some(source);
            }
        }
    }
    let apprentice = game
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
        .ok_or("Sparkmage Apprentice was not cast in the full game")?;
    let mana_paid = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellManaPaid {
            player: PlayerId(0),
            card,
            colors,
        } if *card == apprentice => Some(colors.clone()),
        _ => None,
    });
    let target = selected_target.or_else(|| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::DamageDealtToPermanent {
                source, permanent, ..
            } if *source == apprentice => Some(*permanent),
            _ => None,
        })
    });
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
        apprentice,
        target_source: selected_source,
        target,
        apprentice_zone: game.zone_of(apprentice),
        mana_paid,
        life: [game.players[0].life, game.players[1].life],
    })
}

fn new_rav_game(definition: CardDefinition, mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sparkmage Apprentice missing from RAV definition registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::TriggerRemoved {
        let mut binding = candidate_sparkmage_apprentice::triggered_ability_binding();
        if mutation == Mutation::DamageTwo {
            binding.ability.effects = vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::PlayerOrCreature,
            }];
        }
        triggers.push(binding);
    }
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

fn play_land(view: &GameView, candidate_cast: bool, watchwolf_cast: bool) -> Option<PolicyAction> {
    if view.lands_played != 0 {
        return None;
    }
    let has_color = |color: Color| {
        view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
        })
    };
    let desired = if !candidate_cast && !has_color(Color::Red) {
        Some(Color::Red)
    } else if candidate_cast && !watchwolf_cast && !has_color(Color::Green) {
        Some(Color::Green)
    } else if candidate_cast && !watchwolf_cast && !has_color(Color::White) {
        Some(Color::White)
    } else {
        None
    };
    view.hand
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && desired.is_some_and(|color| card.mana_colors.contains(&color))
        })
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        })
        .map(|card| PolicyAction::PlayLand { card: card.id })
}

fn activate_land_color(view: &GameView, color: Color) -> Option<PolicyAction> {
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

fn activate_any_land(view: &GameView) -> Option<PolicyAction> {
    view.own_battlefield
        .iter()
        .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))
        .and_then(|card| {
            card.mana_colors
                .iter()
                .next()
                .copied()
                .map(|color| (card.id, color))
        })
        .map(|(land, color)| PolicyAction::ActivateManaAbility { land, color })
}

struct SparkmagePolicy {
    player: PlayerId,
    generic_cost_mutation: bool,
    apprentice_cast: bool,
    watchwolf_cast: bool,
    elf_cast: bool,
}

impl SparkmagePolicy {
    fn new(player: PlayerId, generic_cost_mutation: bool) -> Self {
        Self {
            player,
            generic_cost_mutation,
            apprentice_cast: false,
            watchwolf_cast: false,
            elf_cast: false,
        }
    }

    fn target_action(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ETB_ABILITY {
            return None;
        }
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(ELF))
            .map(|card| Target::Permanent(card.id))?;
        if !choice.target_options.first()?.contains(&target) {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![target],
        })
    }

    fn cast_apprentice(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.apprentice_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
            || !view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(ELF))
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if self.generic_cost_mutation {
            if view.mana_pool.amount(Color::Green) == 0 {
                return activate_land_color(view, Color::Green);
            }
            if view.mana_pool.amount(Color::White) == 0 {
                return activate_land_color(view, Color::White);
            }
        } else if view.mana_pool.amount(Color::Red) == 0 {
            return activate_land_color(view, Color::Red);
        }
        if view.mana_pool.total() < 2 {
            return activate_any_land(view);
        }
        let generic = if self.generic_cost_mutation {
            vec![Color::Green, Color::White]
        } else if view.mana_pool.amount(Color::Green) > 0 {
            vec![Color::Green]
        } else if view.mana_pool.amount(Color::White) > 0 {
            vec![Color::White]
        } else if view.mana_pool.amount(Color::Red) > 1 {
            vec![Color::Red]
        } else {
            return activate_any_land(view);
        };
        self.apprentice_cast = true;
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

    fn cast_watchwolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || !self.apprentice_cast
            || self.watchwolf_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return activate_land_color(view, Color::Green);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return activate_land_color(view, Color::White);
        }
        self.watchwolf_cast = true;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
}

impl CodePolicy for SparkmagePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.sparkmage-apprentice-pilot.v1"
        } else {
            "rav.sparkmage-apprentice-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.target_action(view) {
            return action;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignment = if self.player == PlayerId(1) {
                view.combat_attackers
                    .iter()
                    .find(|attacker| attacker.definition == Some(WATCHWOLF))
                    .and_then(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| blocker.definition == Some(ELF) && blocker.can_block)
                            .map(|blocker| CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
            } else {
                None
            };
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
        if let Some(action) = play_land(view, self.apprentice_cast, self.watchwolf_cast) {
            return action;
        }
        if self.player == PlayerId(0) {
            if let Some(action) = self.cast_apprentice(view) {
                return action;
            }
            if let Some(action) = self.cast_watchwolf(view) {
                return action;
            }
        } else {
            if !self.elf_cast {
                if let Some(elf) = view.hand.iter().find(|card| card.definition == Some(ELF)) {
                    if view.mana_pool.amount(Color::Green) == 0 {
                        return activate_land_color(view, Color::Green)
                            .unwrap_or(PolicyAction::PassPriority);
                    }
                    self.elf_cast = true;
                    return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                        card: elf.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err(
            "Sparkmage Apprentice shuffled full game did not end in a seat-zero win".to_owned(),
        );
    }
    if captured.rejected_moves != 0 {
        return Err("Sparkmage Apprentice game has rejected policy moves".to_owned());
    }
    if captured.life[0] <= 0 || captured.life[1] > 0 {
        return Err("Sparkmage Apprentice terminal life state is invalid".to_owned());
    }
    let target = captured
        .target
        .ok_or("Sparkmage Apprentice ETB target was not chosen and confirmed on the stack")?;
    if captured.target_source != Some(captured.apprentice) {
        return Err(
            "Sparkmage Apprentice target selection did not originate from the cast creature"
                .to_owned(),
        );
    }
    if captured.apprentice_zone != Some(Zone::Battlefield) {
        return Err(
            "Sparkmage Apprentice did not remain on the battlefield after its ETB trigger"
                .to_owned(),
        );
    }
    let mana = captured
        .mana_paid
        .as_ref()
        .ok_or("Sparkmage Apprentice exact mana payment receipt is missing")?;
    if mana.len() != 2 || mana.iter().filter(|color| **color == Color::Red).count() != 1 {
        return Err(
            "Sparkmage Apprentice mana payment did not include exactly one red mana symbol"
                .to_owned(),
        );
    }
    if !captured.events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == captured.apprentice && *permanent == target)) {
        return Err("Sparkmage Apprentice ETB trigger did not deal exactly one damage".to_owned());
    }
    if !captured.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)) {
        return Err("Sparkmage Apprentice damage did not remove the target creature".to_owned());
    }
    let mana_paid = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.apprentice)).ok_or("Sparkmage Apprentice mana payment receipt is missing")?;
    let cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.apprentice)).ok_or("Sparkmage Apprentice cast receipt is missing")?;
    let resolved = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.apprentice),
    )
    .ok_or("Sparkmage Apprentice creature spell did not resolve")?;
    let entered = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == captured.apprentice)).ok_or("Sparkmage Apprentice did not enter the battlefield")?;
    let trigger = unique_event_index(&captured.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ETB_ABILITY, .. } if *source == captured.apprentice)).ok_or("Sparkmage Apprentice ETB ability did not enter the stack")?;
    let damage = unique_event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == captured.apprentice && *permanent == target)).ok_or("Sparkmage Apprentice ETB damage event is missing")?;
    let ability_resolved = unique_event_index(&captured.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ETB_ABILITY, .. } if *source == captured.apprentice)).ok_or("Sparkmage Apprentice ETB ability did not resolve")?;
    let target_sba = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::StateBasedAction { card, .. } if *card == target),
    )
    .ok_or("Sparkmage Apprentice lethal damage did not cause a state-based action")?;
    let target_move = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("Sparkmage Apprentice target did not move to its graveyard")?;
    if !(mana_paid < cast
        && cast < resolved
        && resolved < entered
        && entered < trigger
        && trigger < damage
        && damage < ability_resolved
        && ability_resolved < target_sba
        && target_sba < target_move)
    {
        return Err(
            "Sparkmage Apprentice cast, trigger, damage, and graveyard events are out of order"
                .to_owned(),
        );
    }
    let attack = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > target_move)
                .then_some(event)
                .and_then(|event| match event {
                    GameEvent::AttackersDeclared {
                        player: PlayerId(0),
                        attackers,
                    } if !attackers.is_empty() => Some(index),
                    _ => None,
                })
        })
        .ok_or("seat zero did not attack after the Sparkmage Apprentice ETB removal")?;
    let blocks = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > attack)
                .then_some(event)
                .and_then(|event| match event {
                    GameEvent::BlockersDeclared {
                        player: PlayerId(1),
                        assignments,
                    } => Some((index, assignments.is_empty())),
                    _ => None,
                })
        })
        .ok_or("seat one did not reach a blockers declaration after the ETB removal")?;
    if !blocks.1 {
        return Err(
            "the removed only blocker still blocked after Sparkmage Apprentice resolved".to_owned(),
        );
    }
    let combat_damage = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > blocks.0)
                .then_some(event)
                .and_then(|event| match event {
                    GameEvent::DamageDealtToPlayer {
                        player: PlayerId(1),
                        amount,
                        ..
                    } if *amount > 0 => Some(index),
                    _ => None,
                })
        })
        .ok_or("seat zero did not deal combat damage after removing the sole blocker")?;
    let end = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Sparkmage Apprentice full-game terminal event is missing")?;
    if !(blocks.0 < combat_damage && combat_damage < end) {
        return Err(
            "post-ETB blocker removal did not lead to seat-zero combat damage and a win".to_owned(),
        );
    }
    let events: Vec<_> = captured
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            let relevant = match event {
                GameEvent::SpellManaPaid { card, .. }
                | GameEvent::SpellCast { card, .. }
                | GameEvent::SpellResolved { card } => *card == captured.apprentice,
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } => *card == captured.apprentice,
                GameEvent::TriggeredAbilityStacked {
                    source, ability, ..
                } => *source == captured.apprentice && *ability == ETB_ABILITY,
                GameEvent::DamageDealtToPermanent {
                    source, permanent, ..
                } => *source == captured.apprentice && *permanent == target,
                GameEvent::AbilityResolved {
                    source, ability, ..
                } => *source == captured.apprentice && *ability == ETB_ABILITY,
                GameEvent::StateBasedAction { card, .. } => *card == target,
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } => *card == target,
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } => index == attack && !attackers.is_empty(),
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    ..
                } => index == blocks.0,
                GameEvent::DamageDealtToPlayer {
                    player: PlayerId(1),
                    amount,
                    ..
                } => index == combat_damage && *amount > 0,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0)),
                } => true,
                _ => false,
            };
            relevant.then_some(event.clone())
        })
        .collect();
    let projected = project_events(&events);
    Ok(json!({
        "trace_id": "rav-sparkmage-apprentice-etb-clears-only-blocker",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": events.len(),
        "apprentice_object": captured.apprentice.0,
        "target_creature": target.0,
        "target_selection": {
            "source": captured.target_source.expect("validated target source").0,
            "target": target.0,
            "target_definition": ELF,
            "stack_object_verified": true
        },
        "mana_cost": "{1}{R}",
        "mana_paid": mana.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "etb": {"ability": ETB_ABILITY, "target_kind": "opponent-creature", "damage": 1, "target_zone_after_trigger": "Graveyard", "only_blocker_removed_before_combat": true},
        "combat": {"attacked_after_trigger": true, "only_blocker_remained": false, "combat_damage_dealt": true},
        "players_life_at_game_end": [{"seat": 0, "life": captured.life[0]}, {"seat": 1, "life": captured.life[1]}],
        "events": projected
    }))
}

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn mutation_detected(seed: u64, mutation: Mutation, expected: &str) -> bool {
    match run_match(seed, mutation).and_then(|captured| validate_trace(&captured, seed)) {
        Ok(_) => false,
        Err(error) => error.contains(expected),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 144;
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
