//! Full-game RAV Centaur Safeguard task authority.
//!
//! The legal two-player game casts the submitted creature, attacks with it,
//! blocks it with Elves of Deep Shadow, and follows its dies trigger through
//! the optional decision and three-life effect.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckList, Game, GameEvent, GameView, HybridManaSymbol,
    ManaCost, ObjectId, PlayerId, PolicyAction, Step, Zone,
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
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/centaur_safeguard.rs";
const DECK_P0: &str = "rav_selesnya_midrange";
const DECK_P1: &str = "rav_golgari_midrange";
const MAX_TRACE_SEED: u64 = 64;
const CANDIDATE_ID: &str = "RAV-CENTAUR-SAFEGUARD";
const ELVES_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/centaur_safeguard.rs"]
mod candidate_centaur_safeguard;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Centaur Safeguard result: {error}");
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
            eprintln!("Centaur Safeguard authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    validate_candidate_triggered_ability()?;
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    let (seed, accepted, declined) = find_traces()?;
    let accepted_trace = validate_trace(&accepted, seed, true)?;
    let declined_trace = validate_trace(&declined, seed, false)?;
    for (captured, accept_trigger) in [(&accepted, true), (&declined, false)] {
        let replay = run_match(
            seed,
            candidate_centaur_safeguard::definition(),
            false,
            accept_trigger,
        )?;
        if captured.events != replay.events
            || captured.terminal != replay.terminal
            || captured.winner != replay.winner
            || captured.safeguard != replay.safeguard
            || project_events(&captured.events) != project_events(&replay.events)
        {
            return Err("Centaur Safeguard full-game replay was not deterministic".to_owned());
        }
    }
    let mutation_sensitivity = json!({
        "toughness_two_mutation_detected_in_game": mutation_detected(Mutation::Toughness, seed),
        "dies_trigger_removal_detected_in_game": mutation_detected(Mutation::RemoveDiesTrigger, seed),
        "optional_choice_decline_verified": true,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["toughness_two_mutation_detected_in_game"] != true
        || mutation_sensitivity["dies_trigger_removal_detected_in_game"] != true
    {
        return Err("a Centaur Safeguard semantic mutation escaped the trace oracle".to_owned());
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
        "card": {"collector_number": 244, "definition_id": CANDIDATE_ID, "name": "Centaur Safeguard"},
        "candidate": {"kind": candidate_kind(), "card_definition": CANDIDATE_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: green/white hybrid symbols and payment",
                "117: timing and priority",
                "119: life totals and life gain",
                "302.1-302.2 and 601.2: creature spell casting and resolution",
                "603.2 and 603.6c: dies-trigger event and trigger condition",
                "603.5: optional triggered ability resolution",
                "608.2: resolving a triggered ability",
                "704.5f: lethal damage state-based action",
            ],
            "source_contract": "RAV printed identity, 3/1 body, two green/white hybrid symbols, and optional dies life-gain ability",
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
        "traces": [accepted_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Centaur Safeguard source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Centaur Safeguard source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Centaur Safeguard source has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "HybridManaSymbol"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = std::collections::BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err(
                        "Centaur Safeguard source contains an unsupported import".to_owned()
                    );
                };
                imports.extend(paths);
            }
            Item::Fn(item) => {
                if functions.insert(item.sig.ident.to_string(), item).is_some() {
                    return Err("Centaur Safeguard source has a duplicate function".to_owned());
                }
            }
            _ => {
                return Err(
                    "Centaur Safeguard source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || file.items.len() != 4 || functions.len() != 2 {
        return Err(
            "Centaur Safeguard imports or module structure differ from the task contract"
                .to_owned(),
        );
    }
    let function = functions
        .get("definition")
        .ok_or("Centaur Safeguard definition() is missing")?;
    let safe_attrs = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if !safe_attrs
        || !matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
        || function.sig.ident != "definition"
        || function.sig.inputs.len() != 0
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || function.sig.generics.params.len() != 0
        || function.sig.generics.where_clause.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &["CardDefinition"]))
        || function.block.stmts.len() != 1
    {
        return Err(
            "Centaur Safeguard source must define one plain definition() -> CardDefinition"
                .to_owned(),
        );
    }
    let Stmt::Expr(Expr::Struct(card), None) = &function.block.stmts[0] else {
        return Err("Centaur Safeguard definition must return one direct card literal".to_owned());
    };
    if !path_is(&card.path, &["CardDefinition"]) || card.rest.is_some() || card.fields.len() != 13 {
        return Err(
            "Centaur Safeguard definition must return all CardDefinition fields directly"
                .to_owned(),
        );
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Centaur Safeguard fields must be named".to_owned());
        };
        if field.colon_token.is_none()
            || !field.attrs.is_empty()
            || fields.insert(name.to_string(), &field.expr).is_some()
        {
            return Err("Centaur Safeguard has shorthand or duplicate fields".to_owned());
        }
    }
    check_string(fields["id"], CANDIDATE_ID)?;
    check_string(fields["name"], "Centaur Safeguard")?;
    check_string(fields["set_code"], "RAV")?;
    check_mana_cost(fields["mana_cost"])?;
    check_set(fields["colors"], "Color", &["Green", "White"])?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], "CardType", &["Creature"])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_i16(fields["power"], &[2, 3])?;
    check_some_i16(fields["toughness"], &[1, 2])?;
    check_empty_vec(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    let trigger_function = functions
        .get("triggered_ability")
        .ok_or("Centaur Safeguard triggered_ability() is missing")?;
    validate_triggered_ability_source(trigger_function)?;
    Ok(())
}

fn validate_triggered_ability_source(function: &syn::ItemFn) -> Result<(), String> {
    let safe_attrs = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if !safe_attrs
        || !matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
        || function.sig.ident != "triggered_ability"
        || !function.sig.inputs.is_empty()
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &["TriggeredAbilityBinding"]))
        || function.block.stmts.len() != 1
    {
        return Err("Centaur Safeguard source must define plain triggered_ability()".to_owned());
    }
    let Stmt::Expr(Expr::Struct(binding), None) = &function.block.stmts[0] else {
        return Err("Centaur Safeguard trigger must be a direct binding literal".to_owned());
    };
    let binding_fields = checked_struct_fields(binding, &["TriggeredAbilityBinding"], 2)?;
    check_string(binding_fields["card_definition"], CANDIDATE_ID)?;
    let Expr::Struct(ability) = binding_fields["ability"] else {
        return Err("Centaur Safeguard ability must be a direct literal".to_owned());
    };
    let ability_fields = checked_struct_fields(ability, &["TriggeredAbility"], 6)?;
    check_string(ability_fields["id"], "dies-may-gain-three-life")?;
    if !matches!(ability_fields["condition"], Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["TriggerCondition", "Dies"]))
    {
        return Err("Centaur Safeguard trigger must use the Dies condition".to_owned());
    }
    let cost_args = call_args(ability_fields["mana_cost"], &["ManaCost", "new"])
        .ok_or("Centaur Safeguard trigger cost must be zero")?;
    if cost_args.len() != 1
        || !matches!(cost_args[0], Expr::Lit(value) if matches!(&value.lit, Lit::Int(number) if number.base10_parse::<u8>().is_ok_and(|n| n == 0)))
    {
        return Err("Centaur Safeguard trigger cost must be zero".to_owned());
    }
    check_bool(ability_fields["optional"], true)?;
    check_empty_vec(ability_fields["targets"])?;
    let Expr::Macro(effects) = ability_fields["effects"] else {
        return Err("Centaur Safeguard trigger effects must be a literal vector".to_owned());
    };
    if !effects.attrs.is_empty() || !effects.mac.path.is_ident("vec") {
        return Err("Centaur Safeguard trigger effects differ".to_owned());
    }
    let effect: Expr = syn::parse2(effects.mac.tokens.clone())
        .map_err(|_| "Centaur Safeguard trigger effect is malformed".to_owned())?;
    let Expr::Struct(effect) = effect else {
        return Err("Centaur Safeguard trigger must gain three life".to_owned());
    };
    let effect_fields = checked_struct_fields(&effect, &["Effect", "GainLifeController"], 1)?;
    if !matches!(effect_fields["amount"], Expr::Lit(value) if matches!(&value.lit, Lit::Int(number) if number.base10_parse::<u8>().is_ok_and(|n| n == 3)))
    {
        return Err("Centaur Safeguard trigger must gain three life".to_owned());
    }
    Ok(())
}

fn checked_struct_fields<'a>(
    structure: &'a syn::ExprStruct,
    expected_path: &[&str],
    expected_count: usize,
) -> Result<std::collections::BTreeMap<String, &'a Expr>, String> {
    if !path_is(&structure.path, expected_path)
        || structure.fields.len() != expected_count
        || structure.rest.is_some()
    {
        return Err("Centaur Safeguard struct literal differs from its source contract".to_owned());
    }
    let mut fields = std::collections::BTreeMap::new();
    for field in &structure.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Centaur Safeguard struct fields must be named".to_owned());
        };
        if field.colon_token.is_none()
            || !field.attrs.is_empty()
            || fields.insert(name.to_string(), &field.expr).is_some()
        {
            return Err("Centaur Safeguard struct fields differ".to_owned());
        }
    }
    Ok(fields)
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_centaur_safeguard::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CANDIDATE_ID)
        .ok_or("Centaur Safeguard is missing from the executable RAV registry")?;
    let cost = ManaCost::with_hybrid(
        0,
        [],
        [
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
        ],
    );
    if submitted.id != runtime.id
        || submitted.name != "Centaur Safeguard"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != cost
        || submitted.colors != BTreeSet::from([Color::Green, Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
    {
        return Err("RAV-CENTAUR-SAFEGUARD identity or printed characteristics differ".to_owned());
    }
    if submitted.power != Some(3) || submitted.toughness != Some(1) {
        return Err("resolved Centaur Safeguard is not a 3/1".to_owned());
    }
    Ok(())
}

fn validate_candidate_triggered_ability() -> Result<(), String> {
    let submitted = candidate_centaur_safeguard::triggered_ability();
    let runtime = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CANDIDATE_ID)
        .ok_or("Centaur Safeguard triggered ability is missing from the RAV registry")?;
    if submitted != runtime {
        return Err("RAV-CENTAUR-SAFEGUARD dies-trigger binding differs".to_owned());
    }
    Ok(())
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
        && path.segments.iter().zip(expected).all(|(part, name)| {
            part.ident == *name && matches!(&part.arguments, PathArguments::None)
        })
}

fn call_args<'a>(expr: &'a Expr, expected: &[&str]) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    (path.qself.is_none() && path_is(&path.path, expected)).then(|| call.args.iter().collect())
}

fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
    {
        Ok(())
    } else {
        Err(format!(
            "Centaur Safeguard string field must be {expected:?}"
        ))
    }
}

fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Centaur Safeguard basic-land flag differs".to_owned())
    }
}

fn check_set(expr: &Expr, kind: &str, values: &[&str]) -> Result<(), String> {
    let args = call_args(expr, &["BTreeSet", "from"])
        .ok_or("Centaur Safeguard set field is not a literal set")?;
    if args.len() != 1 {
        return Err("Centaur Safeguard set field differs".to_owned());
    }
    let Expr::Array(array) = args[0] else {
        return Err("Centaur Safeguard set field differs".to_owned());
    };
    let actual: Option<Vec<String>> = array
        .elems
        .iter()
        .map(|item| {
            let Expr::Path(path) = item else {
                return None;
            };
            let variant = path.path.segments.last()?.ident.to_string();
            (path.qself.is_none()
                && path.path.segments.len() == 2
                && path.path.segments.first()?.ident == kind
                && values.contains(&variant.as_str()))
            .then_some(variant)
        })
        .collect();
    if actual
        .as_deref()
        .is_some_and(|actual| actual.iter().map(String::as_str).eq(values.iter().copied()))
    {
        Ok(())
    } else {
        Err("Centaur Safeguard set field differs".to_owned())
    }
}

fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_args(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Centaur Safeguard empty set field differs".to_owned())
    }
}

fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    if matches!(expr, Expr::Macro(mac) if mac.attrs.is_empty() && mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty())
    {
        Ok(())
    } else {
        Err("Centaur Safeguard keyword/effect list must be empty".to_owned())
    }
}

fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Centaur Safeguard rules must be a static slice".to_owned());
    };
    let Expr::Array(array) = reference.expr.as_ref() else {
        return Err("Centaur Safeguard rules differ".to_owned());
    };
    let expected = [
        "full-rules-fidelity",
        "hybrid-cost-casting",
        "base-characteristics",
        "optional-dies-gain-three-life",
    ];
    if array.elems.len() == expected.len()
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(expr, expected)| check_string(expr, expected).is_ok())
    {
        Ok(())
    } else {
        Err("Centaur Safeguard rules differ".to_owned())
    }
}

fn check_some_i16(expr: &Expr, allowed: &[i16]) -> Result<(), String> {
    let args = call_args(expr, &["Some"])
        .ok_or("Centaur Safeguard characteristics must be fixed integers")?;
    if args.len() != 1 {
        return Err("Centaur Safeguard characteristics differ".to_owned());
    }
    let Expr::Lit(value) = args[0] else {
        return Err("Centaur Safeguard characteristics differ".to_owned());
    };
    if let Lit::Int(number) = &value.lit
        && number
            .base10_parse::<i16>()
            .is_ok_and(|n| allowed.contains(&n))
    {
        Ok(())
    } else {
        Err("Centaur Safeguard characteristic differs".to_owned())
    }
}

fn check_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_args(expr, &["ManaCost", "with_hybrid"])
        .ok_or("Centaur Safeguard cost must use ManaCost::with_hybrid")?;
    if args.len() != 3
        || !matches!(args[0], Expr::Lit(value) if matches!(&value.lit, Lit::Int(number) if number.base10_parse::<u8>().is_ok_and(|n| n == 0)))
        || !matches!(args[1], Expr::Array(array) if array.elems.is_empty())
    {
        return Err("Centaur Safeguard hybrid cost differs".to_owned());
    }
    let Expr::Array(symbols) = args[2] else {
        return Err("Centaur Safeguard hybrid symbols differ".to_owned());
    };
    if symbols.elems.len() == 2
        && symbols
            .elems
            .iter()
            .all(|symbol| check_hybrid_symbol(symbol).is_ok())
    {
        Ok(())
    } else {
        Err("Centaur Safeguard requires two green/white hybrid symbols".to_owned())
    }
}

fn check_hybrid_symbol(expr: &Expr) -> Result<(), String> {
    let Expr::Struct(symbol) = expr else {
        return Err("Centaur Safeguard hybrid symbol must be a literal".to_owned());
    };
    if !path_is(&symbol.path, &["HybridManaSymbol"])
        || symbol.fields.len() != 2
        || symbol.rest.is_some()
    {
        return Err("Centaur Safeguard hybrid symbol differs".to_owned());
    }
    let mut first = None;
    let mut second = None;
    for field in &symbol.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Centaur Safeguard hybrid symbol fields must be named".to_owned());
        };
        if field.colon_token.is_none() || !field.attrs.is_empty() {
            return Err("Centaur Safeguard hybrid symbol fields differ".to_owned());
        }
        match name.to_string().as_str() {
            "first" if first.is_none() => first = Some(&field.expr),
            "second" if second.is_none() => second = Some(&field.expr),
            _ => return Err("Centaur Safeguard hybrid symbol field name differs".to_owned()),
        }
    }
    check_color(
        first.ok_or("Centaur Safeguard hybrid first color missing")?,
        "Green",
    )?;
    check_color(
        second.ok_or("Centaur Safeguard hybrid second color missing")?,
        "White",
    )
}

fn check_color(expr: &Expr, color: &str) -> Result<(), String> {
    matches!(expr, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["Color", color])).then_some(()).ok_or_else(|| "Centaur Safeguard hybrid color differs".to_owned())
}

fn find_traces() -> Result<(u64, CapturedMatch, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_CENTAUR_SAFEGUARD_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_CENTAUR_SAFEGUARD_SEED must be unsigned".to_owned())?;
        let accepted = run_match(seed, candidate_centaur_safeguard::definition(), false, true)?;
        validate_trace(&accepted, seed, true)?;
        let declined = run_match(
            seed,
            candidate_centaur_safeguard::definition(),
            false,
            false,
        )?;
        validate_trace(&declined, seed, false)?;
        return Ok((seed, accepted, declined));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) =
            run_match(seed, candidate_centaur_safeguard::definition(), false, true)
            && captured.terminal == "Winner(PlayerId(0))"
            && validate_trace(&captured, seed, true).is_ok()
        {
            if let Ok(declined) = run_match(
                seed,
                candidate_centaur_safeguard::definition(),
                false,
                false,
            ) && validate_trace(&declined, seed, false).is_ok()
            {
                return Ok((seed, captured, declined));
            }
        }
    }
    Err(format!(
        "no Centaur Safeguard trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone, Copy)]
enum Mutation {
    Toughness,
    RemoveDiesTrigger,
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    let mut definition = candidate_centaur_safeguard::definition();
    let remove_trigger = matches!(mutation, Mutation::RemoveDiesTrigger);
    if matches!(mutation, Mutation::Toughness) {
        definition.toughness = Some(2);
    }
    let expected = match mutation {
        Mutation::Toughness => "resolved Centaur Safeguard is not a 3/1",
        Mutation::RemoveDiesTrigger => "Centaur Safeguard dies trigger was not stacked after death",
    };
    matches!(run_match(seed, definition, remove_trigger, true).and_then(|captured| validate_trace(&captured, seed, true).map(|_| ())), Err(error) if error.contains(expected))
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    safeguard: Option<ObjectId>,
    resolved_power_toughness: Option<(i32, i32)>,
    resolved_cost: Option<ManaCost>,
    mana_available_at_cast: [u8; 2],
    mana_paid: [u8; 2],
    blocker: Option<ObjectId>,
    optional_choice_seen: bool,
    optional_can_pay: Option<bool>,
    optional_paid: bool,
    life_before_choice: Option<i64>,
    life_after_resolution: Option<i64>,
    source_in_graveyard_before_choice: Option<bool>,
}

fn task_deck(base: &DeckList) -> Result<DeckList, String> {
    let mut deck = base.clone();
    let entry = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-LOXODON-HIERARCH")
        .ok_or("Selesnya deck no longer has its four Loxodon Hierarch slots")?;
    if entry.count != 4 {
        return Err("Selesnya task deck replacement must preserve a four-of slot".to_owned());
    }
    entry.card = CANDIDATE_ID.to_owned();
    let size: usize = deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if size != 60 {
        return Err(format!("Centaur task deck has {size} cards, expected 60"));
    }
    Ok(deck)
}

fn run_match(
    seed: u64,
    definition: CardDefinition,
    remove_trigger: bool,
    accept_trigger: bool,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let source0 = decks
        .iter()
        .find(|deck| deck.id == DECK_P0)
        .ok_or("Selesnya deck missing")?;
    let source1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Golgari deck missing")?;
    let deck0 = task_deck(&source0.deck)?;
    let deck1 = source1.deck.clone();
    let size1: usize = deck1
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if size1 != 60 {
        return Err(format!("{} has {size1} cards, expected 60", DECK_P1));
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_rav_game(definition, remove_trigger)?;
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
        Box::new(CentaurSafeguardPolicy::new(PlayerId(0), accept_trigger)),
        Box::new(GolgariBlockPolicy::new(PlayerId(1))),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut safeguard = None;
    let mut resolved_power_toughness = None;
    let mut resolved_cost = None;
    let mut mana_available_at_cast = [0; 2];
    let mut optional_choice_seen = false;
    let mut optional_can_pay = None;
    let mut optional_paid = false;
    let mut life_before_choice = None;
    let mut life_after_resolution = None;
    let mut source_in_graveyard_before_choice = None;
    let mut blocker = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Centaur Safeguard game hit a run bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let candidate_id = safeguard
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CANDIDATE_ID))
                    .map(|card| card.id)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CANDIDATE_ID))
                    .map(|card| card.id)
            });
        if let Some(choice) = &view.optional_triggered_ability_choice
            && choice.ability == "dies-may-gain-three-life"
            && candidate_id == Some(choice.source)
        {
            optional_choice_seen = true;
            optional_can_pay = Some(choice.can_pay);
            life_before_choice = Some(view.own_life);
            source_in_graveyard_before_choice =
                Some(game.zone_of(choice.source) == Some(Zone::Graveyard));
        }
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
        if matches!(&action, PolicyAction::ResolveOptionalTriggeredAbility { source, ability: "dies-may-gain-three-life", pay: true, .. } if Some(*source) == candidate_id)
        {
            optional_paid = true;
        }
        if let Some(request) = match &action {
            PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } => {
                Some(request)
            }
            _ => None,
        } && view
            .hand
            .iter()
            .any(|card| card.id == request.card && card.definition == Some(CANDIDATE_ID))
        {
            mana_available_at_cast = [
                view.mana_pool.amount(Color::Green),
                view.mana_pool.amount(Color::White),
            ];
        }
        let old_event_count = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("policy move rejected: {error}"))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let new_events = &game.event_log[old_event_count..];
        for event in new_events {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    safeguard = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == safeguard => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    resolved_power_toughness = Some((
                        characteristics.power.ok_or("no live power")?,
                        characteristics.toughness.ok_or("no live toughness")?,
                    ));
                    resolved_cost = Some(
                        game.card_definition(*card)
                            .map_err(|error| error.to_string())?
                            .mana_cost
                            .clone(),
                    );
                }
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    assignments,
                } if blocker.is_none() => {
                    blocker = assignments.iter().find_map(|(attacker, blocker)| {
                        (Some(*attacker) == safeguard
                            && game
                                .card_definition(*blocker)
                                .is_ok_and(|def| def.id == ELVES_ID))
                        .then_some(*blocker)
                    });
                }
                GameEvent::LifeGained {
                    player: PlayerId(0),
                    amount: 3,
                } if optional_choice_seen && life_after_resolution.is_none() => {
                    life_after_resolution = Some(
                        game.view_for_player(PlayerId(0))
                            .map_err(|error| error.to_string())?
                            .own_life,
                    );
                }
                GameEvent::AbilityResolved {
                    source,
                    ability: "dies-may-gain-three-life",
                    ..
                } if Some(*source) == safeguard && life_after_resolution.is_none() => {
                    life_after_resolution = Some(
                        game.view_for_player(PlayerId(0))
                            .map_err(|error| error.to_string())?
                            .own_life,
                    );
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let winner = game.winner().map(|player| player.0);
    let mut mana_paid = [0_u8; 2];
    if let Some(card) = safeguard {
        for event in &events {
            if let GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card: paid,
                colors,
            } = event
                && *paid == card
            {
                for color in colors {
                    match color {
                        Color::Green => mana_paid[0] += 1,
                        Color::White => mana_paid[1] += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(CapturedMatch {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({winner}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        safeguard,
        resolved_power_toughness,
        resolved_cost,
        mana_available_at_cast,
        mana_paid,
        blocker,
        optional_choice_seen,
        optional_can_pay,
        optional_paid,
        life_before_choice,
        life_after_resolution,
        source_in_graveyard_before_choice,
    })
}

fn new_rav_game(definition: CardDefinition, remove_trigger: bool) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CANDIDATE_ID)
        .ok_or("Centaur Safeguard definition missing from RAV registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    if remove_trigger {
        triggers.retain(|binding| {
            !(binding.card_definition == CANDIDATE_ID
                && binding.ability.id == "dies-may-gain-three-life")
        });
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

struct CentaurSafeguardPolicy {
    player: PlayerId,
    accept_trigger: bool,
    inner: RavCatalogPolicy,
}
impl CentaurSafeguardPolicy {
    fn new(player: PlayerId, accept_trigger: bool) -> Self {
        Self {
            player,
            accept_trigger,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}
impl CodePolicy for CentaurSafeguardPolicy {
    fn id(&self) -> &'static str {
        "rav.centaur_safeguard-optional-choice.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
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
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CANDIDATE_ID))
            {
                let green = view.mana_pool.amount(Color::Green);
                let white = view.mana_pool.amount(Color::White);
                let payment = if green >= 2 {
                    Some(vec![Color::Green, Color::Green])
                } else if white >= 2 {
                    Some(vec![Color::White, Color::White])
                } else if green > 0 && white > 0 {
                    Some(vec![Color::Green, Color::White])
                } else {
                    None
                };
                if let Some(hybrid) = payment {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: cardbench_magic_engine::ManaPaymentSelection {
                            generic: vec![],
                            hybrid,
                        },
                    };
                }
                if let Some((land, color)) = view.own_battlefield.iter().find_map(|land| {
                    (!land.tapped && land.card_types.contains(&CardType::Land))
                        .then(|| {
                            [Color::Green, Color::White]
                                .into_iter()
                                .find(|color| land.mana_colors.contains(color))
                                .map(|color| (land.id, color))
                        })
                        .flatten()
                }) {
                    return PolicyAction::ActivateManaAbility { land, color };
                }
            }
        }
        self.inner.propose_move(view)
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("optional choice is visible");
        if choice.ability == "dies-may-gain-three-life" {
            PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: self.accept_trigger,
                target: None,
            }
        } else {
            self.inner.propose_optional_triggered_ability(view)
        }
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
        "rav.centaur_safeguard-golgari-block.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.player == self.player
                    && view.active_player == self.player
                    && !view.attackers_declared =>
            {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            Step::DeclareBlockers
                if view.player == self.player
                    && view.active_player != self.player
                    && !view.blockers_declared =>
            {
                return PolicyAction::DeclareBlockers {
                    assignments: view
                        .combat_attackers
                        .iter()
                        .find(|card| card.definition == Some(CANDIDATE_ID))
                        .and_then(|attacker| {
                            view.own_battlefield
                                .iter()
                                .find(|card| {
                                    card.definition == Some(ELVES_ID)
                                        && card.can_block
                                        && !card.tapped
                                })
                                .map(|blocker| cardbench_magic_engine::CombatBlock {
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
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(ELVES_ID))
                && !view
                    .stack_spells
                    .iter()
                    .any(|card| card.definition == Some(ELVES_ID))
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(ELVES_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                        card: card.id,
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

fn validate_trace(
    captured: &CapturedMatch,
    seed: u64,
    accept_trigger: bool,
) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err("Centaur Safeguard game did not reach a clean seat-zero win".to_owned());
    }
    let safeguard = captured.safeguard.ok_or("Centaur Safeguard was not cast")?;
    if captured.resolved_power_toughness != Some((3, 1)) {
        return Err("resolved Centaur Safeguard is not a 3/1".to_owned());
    }
    let cost = captured
        .resolved_cost
        .as_ref()
        .ok_or("Centaur Safeguard cost missing")?;
    if cost != &candidate_centaur_safeguard::definition().mana_cost {
        return Err("Centaur Safeguard hybrid cost differs".to_owned());
    }
    if captured.mana_available_at_cast[0] + captured.mana_available_at_cast[1] < 2
        || captured.mana_paid.iter().sum::<u8>() != 2
    {
        return Err("Centaur Safeguard did not pay two green/white hybrid symbols".to_owned());
    }
    let source = captured
        .blocker
        .ok_or("Elves of Deep Shadow did not block Centaur Safeguard")?;
    if !captured.events.iter().any(|event| matches!(event,
        GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: trigger_source, ability: "dies-may-gain-three-life", .. }
            if *trigger_source == safeguard))
    {
        return Err("Centaur Safeguard dies trigger was not stacked after death".to_owned());
    }
    if !captured.events.iter().any(|event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source: blocker, permanent, amount: 1 }
            if *blocker == source && *permanent == safeguard)
    }) {
        return Err("Elves of Deep Shadow did not deal its normal combat damage".to_owned());
    }
    if captured.blocker.is_none()
        || !captured.optional_choice_seen
        || captured.optional_can_pay != Some(true)
        || captured.optional_paid != accept_trigger
        || captured.source_in_graveyard_before_choice != Some(true)
    {
        return Err(
            "Centaur Safeguard dies trigger did not reach and accept its optional decision"
                .to_owned(),
        );
    }
    let positions = event_positions(&captured.events, safeguard, source, accept_trigger)?;
    if !(positions.cast < positions.resolved
        && positions.resolved < positions.entry
        && positions.entry < positions.attack
        && positions.attack < positions.block
        && positions.block < positions.combat_step
        && positions.combat_step < positions.source_damage
        && positions.source_damage < positions.blocker_damage
        && positions.blocker_damage < positions.source_death
        && positions.source_death < positions.source_move
        && positions.source_move < positions.trigger
        && positions.trigger < positions.ability_resolved
        && positions.ability_resolved < positions.optional_move
        && positions.optional_move < positions.game_end)
    {
        return Err(
            "Centaur Safeguard cast, death, trigger, or life-gain order differs".to_owned(),
        );
    }
    let life_gained = if accept_trigger { 3 } else { 0 };
    if captured
        .life_before_choice
        .zip(captured.life_after_resolution)
        .is_none_or(|(before, after)| after - before != life_gained)
        || positions.life.is_some() != accept_trigger
    {
        return Err(
            "Centaur Safeguard optional trigger did not gain exactly three life".to_owned(),
        );
    }
    let projection = project_events(&captured.events);
    let mut selected = vec![
        positions.payment,
        positions.cast,
        positions.resolved,
        positions.entry,
        positions.attack,
        positions.block,
        positions.combat_step,
        positions.source_damage,
        positions.blocker_damage,
        positions.source_death,
        positions.source_move,
        positions.trigger,
    ];
    if let Some(life) = positions.life {
        selected.push(life);
    }
    selected.extend([
        positions.ability_resolved,
        positions.optional_move,
        positions.game_end,
    ]);
    let projected: Vec<_> = selected.iter().map(|i| projection[*i].clone()).collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    let mut expected = vec![
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "StepBegan",
        "DamageDealtToPermanent",
        "DamageDealtToPermanent",
        "StateBasedAction",
        "CardMoved",
        "TriggeredAbilityStacked",
    ];
    if accept_trigger {
        expected.push("LifeGained");
    }
    expected.extend(["AbilityResolved", "PolicyMoveSubmitted", "GameEnded"]);
    if kinds != expected {
        return Err(format!("Centaur Safeguard event trace differs: {kinds:?}"));
    }
    Ok(json!({
        "id": if accept_trigger { "rav-centaur-safeguard-optional-dies-life-gain-accepted" } else { "rav-centaur-safeguard-optional-dies-life-gain-declined" },
        "seed": seed,
        "decks": [format!("{DECK_P0}+4-centaur-safeguards"), DECK_P1],
        "policies": ["rav.centaur_safeguard-optional-choice.v1", "rav.centaur_safeguard-golgari-block.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "centaur_safeguard_object": safeguard.0,
        "mana_paid": {"green": captured.mana_paid[0], "white": captured.mana_paid[1], "total": captured.mana_paid.iter().sum::<u8>()},
        "resolved_characteristics": {"power": 3, "toughness": 1},
        "optional_trigger": {"source_object": safeguard.0, "blocker_object": source.0, "choice_seen": captured.optional_choice_seen, "can_pay": captured.optional_can_pay, "choice": if accept_trigger { "accepted" } else { "declined" }, "paid": captured.optional_paid, "source_in_graveyard_before_choice": captured.source_in_graveyard_before_choice, "life_before": captured.life_before_choice, "life_after": captured.life_after_resolution, "life_gained": life_gained},
        "events": projected,
        "passed": true
    }))
}

struct Positions {
    payment: usize,
    cast: usize,
    resolved: usize,
    entry: usize,
    attack: usize,
    block: usize,
    combat_step: usize,
    source_damage: usize,
    blocker_damage: usize,
    source_death: usize,
    source_move: usize,
    trigger: usize,
    optional_move: usize,
    life: Option<usize>,
    ability_resolved: usize,
    game_end: usize,
}
fn event_positions(
    events: &[GameEvent],
    source: ObjectId,
    blocker: ObjectId,
    accept_trigger: bool,
) -> Result<Positions, String> {
    let pos = |predicate: &dyn Fn(&GameEvent) -> bool, start: usize, label: &str| {
        events
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(i, e)| predicate(e).then_some(i))
            .ok_or_else(|| format!("{label} event missing"))
    };
    let payment = pos(
        &|e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == source),
        0,
        "Centaur Safeguard payment",
    )?;
    let cast = pos(
        &|e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source),
        payment + 1,
        "Centaur Safeguard cast",
    )?;
    let resolved = pos(
        &|e| matches!(e, GameEvent::SpellResolved { card } if *card == source),
        cast + 1,
        "Centaur Safeguard resolution",
    )?;
    let entry = pos(
        &|e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source),
        resolved + 1,
        "Centaur Safeguard battlefield entry",
    )?;
    let attack = pos(
        &|e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source)),
        entry + 1,
        "Centaur Safeguard attack",
    )?;
    let block = pos(
        &|e| matches!(e, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(source, blocker))),
        attack + 1,
        "Centaur Safeguard block",
    )?;
    let combat_step = pos(
        &|e| {
            matches!(
                e,
                GameEvent::StepBegan {
                    active_player: PlayerId(0),
                    step: Step::CombatDamage,
                    ..
                }
            )
        },
        block + 1,
        "CombatDamage step",
    )?;
    let source_damage = pos(
        &|e| matches!(e, GameEvent::DamageDealtToPermanent { source: attacker, permanent, amount: 3 } if *attacker == source && *permanent == blocker),
        combat_step + 1,
        "Centaur Safeguard combat damage",
    )?;
    let blocker_damage = pos(
        &|e| matches!(e, GameEvent::DamageDealtToPermanent { source: blocker_source, permanent, amount: 1 } if *blocker_source == blocker && *permanent == source),
        source_damage + 1,
        "Elves of Deep Shadow combat damage",
    )?;
    let source_death = pos(
        &|e| matches!(e, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == source),
        blocker_damage + 1,
        "Centaur Safeguard lethal-damage state action",
    )?;
    let source_move = pos(
        &|e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source),
        source_death + 1,
        "Centaur Safeguard graveyard move",
    )?;
    let trigger = pos(
        &|e| matches!(e, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: trigger_source, ability: "dies-may-gain-three-life", .. } if *trigger_source == source),
        source_move + 1,
        "Centaur Safeguard dies trigger",
    )?;
    let ability_resolved = pos(
        &|e| matches!(e, GameEvent::AbilityResolved { source: resolved_source, ability: "dies-may-gain-three-life", .. } if *resolved_source == source),
        trigger + 1,
        "Centaur Safeguard ability resolution",
    )?;
    let life = events
        .iter()
        .enumerate()
        .skip(trigger + 1)
        .take(ability_resolved.saturating_sub(trigger + 1))
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::LifeGained {
                    player: PlayerId(0),
                    amount: 3
                }
            )
            .then_some(index)
        });
    if accept_trigger && life.is_none() {
        return Err("Centaur Safeguard accepted trigger has no three-life result".to_owned());
    }
    if !accept_trigger && life.is_some() {
        return Err("Centaur Safeguard declined trigger unexpectedly gained life".to_owned());
    }
    let optional_move = pos(
        &|e| matches!(e, GameEvent::PolicyMoveSubmitted { player: PlayerId(0), policy, kind } if policy == "rav.centaur_safeguard-optional-choice.v1" && *kind == cardbench_magic_engine::PolicyMoveKind::ResolveOptionalTriggeredAbility),
        ability_resolved + 1,
        "optional triggered ability decision",
    )?;
    let game_end = pos(
        &|e| {
            matches!(
                e,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        },
        optional_move + 1,
        "terminal win",
    )?;
    Ok(Positions {
        payment,
        cast,
        resolved,
        entry,
        attack,
        block,
        combat_step,
        source_damage,
        blocker_damage,
        source_death,
        source_move,
        trigger,
        optional_move,
        life,
        ability_resolved,
        game_end,
    })
}

fn event_digest(events: &[GameEvent]) -> String {
    let mut text = String::new();
    for event in events {
        use std::fmt::Write as _;
        let _ = writeln!(text, "{event:?}");
    }
    let hash = text
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
    let passed = definition_id == Some(CANDIDATE_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status {CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
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
