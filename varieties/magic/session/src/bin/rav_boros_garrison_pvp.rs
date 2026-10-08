//! Full-game diagnostic authority for RAV Boros Garrison.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    LandEntryBinding, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement,
    TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_land_entry_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BOROS-GARRISON";
const CARD_NAME: &str = "Boros Garrison";
const ABILITY_ID: &str = "return-controlled-land";
const MANA_ABILITY_ID: &str = "boros-garrison-rw";
const CREATURE_ID: &str = "RAV-SKYKNIGHT-LEGIONNAIRE";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs";
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 4_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs"]
mod candidate_boros_garrison;

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
            eprintln!("Boros Garrison PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Boros Garrison full-game replay differs".to_owned());
    }
    let mutations = json!({
        "enters_tapped_removed_detected": mutation_detected(seed, Mutation::Untapped)?,
        "controlled_land_target_restriction_changed_detected": mutation_detected(seed, Mutation::WrongTarget)?,
        "return_trigger_removed_detected": mutation_detected(seed, Mutation::NoTrigger)?,
        "red_white_mana_bundle_changed_detected": mutation_detected(seed, Mutation::WrongBundle)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "enters_tapped_removed_detected",
        "controlled_land_target_restriction_changed_detected",
        "return_trigger_removed_detected",
        "red_white_mana_bundle_changed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Boros Garrison gameplay mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":275,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["305.1 and 305.2: playing lands and land characteristics", "603.2-603.3: triggered ability occurrence and stacking", "601.2 and 608: targets and resolution", "701.17: returning a permanent", "106.4-106.5: mana abilities and produced mana", "508-510: combat damage", "704: state-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Boros Garrison #275: a land that enters tapped, triggers its controller to return a controlled land to its owner's hand, and taps for {R}{W}.",
            "authority_cutoff":"2026-09-23","no_provider_trial":true
        },
        "passed":cards_passed == 1,"pilot_card_passed":cards_passed == 1,"cards_passed":cards_passed,
        "cards_credited":0,"cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","replay_deterministic":true,
        "mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !unique
            .iter()
            .any(|card| card.collector_number == 275 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Boros Garrison".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Boros Garrison source: {error}"))?;
    if source.len() > 64 * 1024 {
        return Err("Boros Garrison source exceeds 64 KiB".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Boros Garrison source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Boros Garrison source has a non-documentation module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedManaAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "LandEntryBinding"],
        vec!["cardbench_magic_engine", "ManaAbilityBinding"],
        vec!["cardbench_magic_engine", "ManaAbilityOutput"],
        vec!["cardbench_magic_engine", "ManaBundle"],
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
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Boros Garrison source contains an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(function) if function.attrs.iter().all(is_safe_function_attribute) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) {
                    return Err("Boros Garrison source has duplicate factories".to_owned());
                }
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "land_entry_binding" => "LandEntryBinding",
                    "mana_ability" => "ManaAbilityBinding",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => {
                        return Err(
                            "Boros Garrison source contains an unexpected function".to_owned()
                        );
                    }
                };
                validate_candidate_function(function, expected_type)?;
                if function.block.stmts.len() != 1 {
                    return Err(
                        "Boros Garrison factories must be direct declarative literals".to_owned(),
                    );
                }
                let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
                    return Err("Boros Garrison factories cannot contain statements".to_owned());
                };
                validate_literal_shape(expression, expected_type)?;
                if !safe_candidate_expression(expression) {
                    return Err(
                        "Boros Garrison source contains an executable expression".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "Boros Garrison source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    let expected_functions = BTreeSet::from([
        "definition".to_owned(),
        "land_entry_binding".to_owned(),
        "mana_ability".to_owned(),
        "triggered_ability".to_owned(),
    ]);
    if imports != expected_imports || functions != expected_functions || file.items.len() != 6 {
        return Err(
            "Boros Garrison imports or module structure differ from the task boundary".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_function(function: &syn::ItemFn, return_type: &str) -> Result<(), String> {
    let signature = &function.sig;
    let visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    let returns_expected = matches!(&signature.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(return_type)));
    if !visibility
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !returns_expected
    {
        return Err(format!(
            "Boros Garrison factory must be a plain {}() -> {}",
            signature.ident, return_type
        ));
    }
    Ok(())
}

fn validate_literal_shape(expression: &Expr, expected: &str) -> Result<(), String> {
    let Expr::Struct(structure) = expression else {
        return Err(format!(
            "Boros Garrison {expected} must return a struct literal"
        ));
    };
    if structure.path.leading_colon.is_some()
        || structure.path.segments.len() != 1
        || structure
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != expected)
        || structure.rest.is_some()
        || !candidate_struct_fields_valid(structure)
    {
        return Err(format!(
            "Boros Garrison literal must be a complete {expected}"
        ));
    }
    Ok(())
}

fn candidate_struct_fields_valid(structure: &syn::ExprStruct) -> bool {
    let expected: BTreeSet<&str> = match structure
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .as_deref()
    {
        Some("CardDefinition") => [
            "id",
            "name",
            "set_code",
            "mana_cost",
            "colors",
            "mana_colors",
            "card_types",
            "is_basic_land",
            "supported_rules",
            "power",
            "toughness",
            "keywords",
            "effects",
        ]
        .into_iter()
        .collect(),
        Some("LandEntryBinding") => ["card_definition", "enters_tapped", "optional_life_payment"]
            .into_iter()
            .collect(),
        Some("ManaAbilityBinding") => ["card_definition", "ability"].into_iter().collect(),
        Some("ActivatedManaAbility") => [
            "id",
            "tap_cost",
            "output",
            "amount",
            "life_payment",
            "controller_damage",
        ]
        .into_iter()
        .collect(),
        Some("TriggeredAbilityBinding") => ["card_definition", "ability"].into_iter().collect(),
        Some("TriggeredAbility") => [
            "id",
            "condition",
            "mana_cost",
            "optional",
            "targets",
            "effects",
        ]
        .into_iter()
        .collect(),
        _ => return false,
    };
    let actual: BTreeSet<String> = structure
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            syn::Member::Unnamed(_) => None,
        })
        .collect();
    actual.len() == structure.fields.len()
        && actual == expected.iter().map(|field| (*field).to_owned()).collect()
        && structure.fields.iter().all(|field| field.attrs.is_empty())
}

fn safe_candidate_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => path.attrs.is_empty() && path.qself.is_none(),
        Expr::Lit(literal) => {
            literal.attrs.is_empty()
                && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_))
        }
        Expr::Array(array) => {
            array.attrs.is_empty() && array.elems.iter().all(safe_candidate_expression)
        }
        Expr::Tuple(tuple) => {
            tuple.attrs.is_empty() && tuple.elems.iter().all(safe_candidate_expression)
        }
        Expr::Reference(reference) => {
            reference.attrs.is_empty() && safe_candidate_expression(&reference.expr)
        }
        Expr::Paren(paren) => paren.attrs.is_empty() && safe_candidate_expression(&paren.expr),
        Expr::Group(group) => group.attrs.is_empty() && safe_candidate_expression(&group.expr),
        Expr::Call(call) => {
            if !call.attrs.is_empty() {
                return false;
            }
            let Expr::Path(function) = &*call.func else {
                return false;
            };
            let path = function
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            matches!(
                path.as_str(),
                "BTreeSet::from"
                    | "BTreeSet::new"
                    | "ManaCost::new"
                    | "ManaBundle::new"
                    | "ManaAbilityOutput::Bundle"
            ) && call.args.iter().all(safe_candidate_expression)
        }
        Expr::Struct(structure) => {
            structure.attrs.is_empty()
                && structure.path.leading_colon.is_none()
                && structure.path.segments.len() == 1
                && structure.rest.is_none()
                && candidate_struct_fields_valid(structure)
                && structure
                    .fields
                    .iter()
                    .all(|field| safe_candidate_expression(&field.expr))
        }
        Expr::Macro(mac) => {
            if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
                return false;
            }
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|expressions| expressions.iter().all(safe_candidate_expression))
        }
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(&literal.lit, Lit::Str(_))))
}

fn is_safe_function_attribute(attribute: &syn::Attribute) -> bool {
    is_doc_attribute(attribute)
        || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
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
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn validate_candidate_semantics() -> Result<(), String> {
    let candidate = candidate_boros_garrison::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Boros Garrison is absent from the RAV registry")?;
    let entry = candidate_boros_garrison::land_entry_binding();
    let registered_entry = rav_land_entry_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Boros Garrison land-entry binding is absent")?;
    let mana = candidate_boros_garrison::mana_ability();
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Boros Garrison mana ability is absent")?;
    let trigger = candidate_boros_garrison::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Boros Garrison return trigger is absent")?;
    let output_matches = matches!(&mana.ability.output, ManaAbilityOutput::Bundle(bundle) if bundle.amount(Color::Red) == 1 && bundle.amount(Color::White) == 1 && bundle.iter().count() == 2);
    let mut mismatches = Vec::new();
    if candidate != registered {
        mismatches.push("definition registry delegation");
    }
    if candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::new(0)
        || !candidate.colors.is_empty()
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Land])
        || candidate.is_basic_land
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || !candidate.effects.is_empty()
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "enters-tapped",
                "etb-return-controlled-land",
                "fixed-red-white-mana-bundle",
            ]
    {
        mismatches.push("land definition fields");
    }
    if entry != registered_entry {
        mismatches.push("land-entry registry delegation");
    }
    if !entry.enters_tapped || entry.optional_life_payment.is_some() {
        mismatches.push("tapped-entry binding");
    }
    if mana != registered_mana {
        mismatches.push("mana registry delegation");
    }
    if mana.ability.id != MANA_ABILITY_ID
        || !mana.ability.tap_cost
        || !output_matches
        || mana.ability.amount != 0
        || mana.ability.life_payment.is_some()
        || mana.ability.controller_damage.is_some()
    {
        mismatches.push("red-white mana ability");
    }
    if trigger != registered_trigger {
        mismatches.push("trigger registry delegation");
    }
    if trigger.ability.id != ABILITY_ID
        || trigger.ability.condition != TriggerCondition::EntersBattlefield
        || trigger.ability.optional
        || trigger.ability.targets != [TargetRequirement::ControlledLand]
        || trigger.ability.effects != [Effect::ReturnControlledLandToHand]
    {
        mismatches.push("mandatory controlled-land return trigger");
    }
    if !mismatches.is_empty() {
        return Err(format!(
            "Boros Garrison candidate or registry differs from RAV #275 contract: {}",
            mismatches.join(", ")
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    Untapped,
    WrongTarget,
    NoTrigger,
    WrongBundle,
}

fn definitions_with_mutation(_mutation: Mutation) -> Vec<CardDefinition> {
    card_definitions()
}

fn entries_with_mutation(mutation: Mutation) -> Vec<LandEntryBinding> {
    let mut entries = rav_land_entry_bindings();
    if mutation == Mutation::Untapped {
        entries.retain(|binding| binding.card_definition != CARD_ID);
    }
    entries
}

fn mana_with_mutation(mutation: Mutation) -> Vec<ManaAbilityBinding> {
    let mut abilities = rav_mana_ability_bindings();
    if mutation == Mutation::WrongBundle {
        if let Some(binding) = abilities
            .iter_mut()
            .find(|binding| binding.card_definition == CARD_ID)
        {
            binding.ability.output =
                ManaAbilityOutput::Bundle(ManaBundle::new([(Color::Blue, 1), (Color::White, 1)]));
        }
    }
    abilities
}

fn triggers_with_mutation(mutation: Mutation) -> Vec<TriggeredAbilityBinding> {
    let mut triggers = rav_triggered_ability_bindings();
    if mutation == Mutation::NoTrigger {
        triggers.retain(|binding| binding.card_definition != CARD_ID);
    }
    if mutation == Mutation::WrongTarget {
        if let Some(binding) = triggers
            .iter_mut()
            .find(|binding| binding.card_definition == CARD_ID)
        {
            binding.ability.targets = vec![TargetRequirement::ControlledCreature];
            binding.ability.effects = vec![Effect::ReturnControlledCreatureToHand];
        }
    }
    triggers
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    garrison: Option<ObjectId>,
    target: Option<ObjectId>,
    target_controller: Option<usize>,
    target_owner: Option<usize>,
    target_definition: Option<&'static str>,
    garrison_tapped: Option<bool>,
    target_returned_to_hand: bool,
    target_final_zone: Option<Zone>,
    bundle: [u8; 2],
    bundle_tapped: bool,
    bundle_mana_pool: [u8; 2],
    creature: Option<ObjectId>,
    creature_zone: Option<Zone>,
    life_totals: [i64; 2],
}

fn find_reference() -> Result<(u64, Captured), String> {
    let seed = 1;
    let capture = run_match(seed, Mutation::None)?;
    validate_trace(&capture, seed)?;
    Ok((seed, capture))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let capture = run_match(seed, mutation).map_err(|error| {
        format!("Boros Garrison mutation {mutation:?} interrupted the full match: {error}")
    })?;
    if capture.rejected_moves != 0 || capture.terminal != "Winner(PlayerId(0))" {
        return Err(format!(
            "Boros Garrison mutation {mutation:?} did not preserve a clean terminal duel"
        ));
    }
    Ok(validate_trace(&capture, seed).is_err())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CREATURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Boros Garrison pilot", &pilot_deck),
        ("land-only opponent", &opponent_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions_with_mutation(mutation),
        2,
        mana_with_mutation(mutation),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers_with_mutation(mutation),
        rav_static_continuous_effect_bindings(),
        entries_with_mutation(mutation),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GarrisonPolicy {
            player: PlayerId(0),
            garrison_played: false,
            bundle_activated_turn: None,
        }),
        Box::new(PassivePolicy {
            player: PlayerId(1),
        }),
    ];
    let mut capture = Captured {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        garrison: None,
        target: None,
        target_controller: None,
        target_owner: None,
        target_definition: None,
        garrison_tapped: None,
        target_returned_to_hand: false,
        target_final_zone: None,
        bundle: [0, 0],
        bundle_tapped: false,
        bundle_mana_pool: [0, 0],
        creature: None,
        creature_zone: None,
        life_totals: [0, 0],
    };
    let mut attempts = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || capture.accepted_moves >= MAX_MOVES {
            return Err(format!(
                "Boros Garrison duel exceeded bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if view.pending_decision.is_some() {
            policy
                .propose_pending_decision(&view)
                .ok_or("triggered ability target choice received no response")?
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
        if let PolicyAction::ChooseTriggeredAbilityTargets {
            source, targets, ..
        } = &action
            && game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            if let Some(Target::Permanent(target)) = targets.first() {
                capture.target = Some(*target);
                capture.target_controller = game.controller_of(*target).ok().map(|owner| owner.0);
                capture.target_owner = game.object(*target).ok().map(|object| object.owner.0);
                capture.target_definition = game
                    .card_definition(*target)
                    .ok()
                    .map(|definition| definition.id);
            }
        }
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy action rejected turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID)
            {
                capture.garrison = Some(*card);
                capture.garrison_tapped = game.object(*card).ok().map(|object| object.tapped);
            }
            if let GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } = event
                && Some(*card) == capture.target
            {
                capture.target_returned_to_hand = true;
                capture.target_final_zone = game.zone_of(*card);
            }
            if let GameEvent::BoundManaAbilityFreeBundleActivated {
                source,
                ability,
                bundle,
                tapped,
                ..
            } = event
                && Some(*source) == capture.garrison
                && *ability == MANA_ABILITY_ID
            {
                capture.bundle = [bundle.amount(Color::Red), bundle.amount(Color::White)];
                capture.bundle_tapped = *tapped;
                let pool = &game
                    .player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .mana_pool;
                capture.bundle_mana_pool = [pool.amount(Color::Red), pool.amount(Color::White)];
            }
            if let GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CREATURE_ID)
            {
                capture.creature = Some(*card);
            }
        }
    }
    capture.events = game.event_log.clone();
    capture.terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |winner| format!("Winner(PlayerId({}))", winner.0),
    );
    capture.winner = game.winner().map(|winner| winner.0);
    capture.turns = game.turn;
    capture.rejected_moves = attempts.saturating_sub(capture.accepted_moves);
    capture.creature_zone = capture.creature.and_then(|creature| game.zone_of(creature));
    capture.life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

struct GarrisonPolicy {
    player: PlayerId,
    garrison_played: bool,
    bundle_activated_turn: Option<u32>,
}
struct PassivePolicy {
    player: PlayerId,
}

impl GarrisonPolicy {
    fn target_action(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ABILITY_ID {
            return None;
        }
        let target = choice
            .target_options
            .first()?
            .iter()
            .find_map(|target| match target {
                Target::Permanent(id)
                    if view.own_battlefield.iter().any(|card| {
                        card.id == *id
                            && card.definition != Some(CARD_ID)
                            && card.card_types.contains(&CardType::Land)
                    }) =>
                {
                    Some(*target)
                }
                _ => None,
            })
            .or_else(|| choice.target_options.first()?.first().copied())?;
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![target],
        })
    }
}

impl CodePolicy for GarrisonPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-garrison-rw-bounce-pilot.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CREATURE_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = choose_pilot_land(view, self.garrison_played)
            {
                if land.definition == Some(CARD_ID) {
                    self.garrison_played = true;
                }
                return PolicyAction::PlayLand { card: land.id };
            }
            if let Some(creature) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CREATURE_ID))
            {
                let cost = ManaCost::with_colors(1, [Color::Red, Color::White]);
                if self.bundle_activated_turn == Some(view.turn)
                    && let Some(mana_selection) = Self::payment(view, &cost)
                {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: creature.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if self.bundle_activated_turn != Some(view.turn)
                    && let Some(garrison) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID) && !card.tapped)
                    && view.own_battlefield.iter().any(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.definition != Some(CARD_ID)
                            && !card.tapped
                    })
                {
                    self.bundle_activated_turn = Some(view.turn);
                    return PolicyAction::ActivateBoundManaAbility {
                        activation: cardbench_magic_engine::ManaAbilityActivation {
                            source: garrison.id,
                            ability_id: MANA_ABILITY_ID,
                            chosen_color: None,
                        },
                    };
                }
                if self.bundle_activated_turn == Some(view.turn)
                    && let Some(land) = view.own_battlefield.iter().find(|card| {
                        !card.tapped
                            && card.card_types.contains(&CardType::Land)
                            && card.definition != Some(CARD_ID)
                    })
                {
                    let color = match land.basic_land_type {
                        Some(cardbench_magic_engine::BasicLandType::Plains) => Color::White,
                        Some(cardbench_magic_engine::BasicLandType::Mountain) => Color::Red,
                        _ => return PolicyAction::PassPriority,
                    };
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color,
                    };
                }
            }
            if view.lands_played == 0
                && let Some(land) = choose_pilot_land(view, self.garrison_played)
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.target_action(view)
    }
}

impl GarrisonPolicy {
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(available_color, _)| available_color == color)?;
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
}

impl CodePolicy for PassivePolicy {
    fn id(&self) -> &'static str {
        "rav.land-only-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        PolicyAction::PassPriority
    }
}

fn choose_pilot_land(
    view: &GameView,
    garrison_played: bool,
) -> Option<&cardbench_magic_engine::CardView> {
    let basic = view
        .hand
        .iter()
        .find(|card| matches!(card.definition, Some("RAV-PLAINS" | "RAV-MOUNTAIN")));
    if garrison_played {
        return basic;
    }
    let garrison = view
        .hand
        .iter()
        .find(|card| card.definition == Some(CARD_ID));
    if view
        .own_battlefield
        .iter()
        .any(|card| card.card_types.contains(&CardType::Land))
    {
        garrison.or(basic)
    } else {
        basic.or(garrison)
    }
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.rejected_moves != 0
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
    {
        return Err(format!(
            "Boros Garrison duel did not reach clean pilot victory: {} {:?}",
            c.terminal, c.life_totals
        ));
    }
    let garrison = c
        .garrison
        .ok_or("Boros Garrison never entered battlefield")?;
    let target = c.target.ok_or("controlled-land target selection missing")?;
    if c.garrison_tapped != Some(true)
        || c.target_controller != Some(0)
        || c.target_owner != Some(0)
        || !matches!(c.target_definition, Some("RAV-PLAINS" | "RAV-MOUNTAIN"))
        || !c.target_returned_to_hand
        || c.target_final_zone != Some(Zone::Hand)
    {
        return Err(format!(
            "Boros Garrison entry or return mismatch: tapped={:?}, target controller/owner={:?}/{:?}, target={:?}, returned={:?}/{:?}",
            c.garrison_tapped,
            c.target_controller,
            c.target_owner,
            c.target_definition,
            c.target_returned_to_hand,
            c.target_final_zone
        ));
    }
    if c.bundle != [1, 1]
        || !c.bundle_tapped
        || c.bundle_mana_pool[0] < 1
        || c.bundle_mana_pool[1] < 1
    {
        return Err(format!(
            "Boros Garrison did not tap for exact {{R}}{{W}} bundle: {:?} pool {:?}",
            c.bundle, c.bundle_mana_pool
        ));
    }
    let creature = c.creature.ok_or("Skyknight Legionnaire was not cast")?;
    if c.creature_zone != Some(Zone::Battlefield) {
        return Err("Skyknight Legionnaire left the battlefield before terminal win".to_owned());
    }
    let moved_in = event_index(&c.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == garrison)).ok_or("Garrison battlefield entry receipt missing")?;
    let trigger = event_index_after(&c.events, moved_in, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == garrison)).ok_or("Garrison return-land ETB trigger was not stacked")?;
    let returned = event_index_after(
        &c.events,
        trigger,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == target),
    )
    .ok_or("exact controlled land target did not return to hand")?;
    let resolved = event_index_after(&c.events, returned, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == garrison)).ok_or("Garrison ETB trigger did not resolve")?;
    let bundle = event_index_after(&c.events, resolved, |event| matches!(event, GameEvent::BoundManaAbilityFreeBundleActivated { player: PlayerId(0), source, ability: MANA_ABILITY_ID, bundle, tapped: true, .. } if *source == garrison && bundle.amount(Color::Red) == 1 && bundle.amount(Color::White) == 1 && bundle.iter().count() == 2)).ok_or("Garrison did not tap for one red and one white mana")?;
    let payment = event_index_after(&c.events, bundle, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == creature && colors.len() == 3 && colors.contains(&Color::Red) && colors.contains(&Color::White))).ok_or("Skyknight Legionnaire did not spend the red-white bundle plus generic mana")?;
    let cast = event_index_after(&c.events, payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == creature)).ok_or("Skyknight Legionnaire did not cast after the bundle activation")?;
    let attack = event_index_after(&c.events, cast, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&creature))).ok_or("Skyknight Legionnaire did not attack")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == creature)).ok_or("Skyknight Legionnaire did not deal combat damage to the opponent")?;
    let terminal = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal pilot victory receipt missing")?;
    if !(moved_in < trigger
        && trigger < returned
        && returned < resolved
        && resolved < bundle
        && bundle < payment
        && payment < cast
        && cast < attack
        && attack < damage
        && damage < terminal)
    {
        return Err("Boros Garrison entry, trigger, mana payment, combat, and terminal receipts are out of order".to_owned());
    }
    let order = [
        moved_in, trigger, returned, resolved, bundle, payment, cast, attack, damage, terminal,
    ];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&c.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-boros-garrison-tapped-bounce-bundle-pvp","seed":seed,
        "decks":["rav-boros-garrison-skyknight-60","rav-land-only-opponent-60"],
        "policies":["rav.boros-garrison-rw-bounce-pilot.v1","rav.land-only-opponent.v1"],
        "terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "mechanics":{"land":garrison.0,"enters_tapped":true,"target":target.0,"target_controller":c.target_controller,"target_owner":c.target_owner,"target_definition":c.target_definition,"target_returns_to_hand":c.target_returned_to_hand,"target_zone_when_returned":format!("{:?}",c.target_final_zone),"mana_ability":MANA_ABILITY_ID,"bundle":["Red","White"],"bundle_tapped_source":true,"mana_pool_at_activation":c.bundle_mana_pool,"skyknight":creature.0,"skyknight_attacked":true,"skyknight_paid_red_white_and_generic":true,"terminal_pilot_win":c.winner == Some(0)},
        "ordered_event_sequence":order,"events":events,"passed":true
    }))
}

fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed { "passed" } else { "not_qualified" },"credited":false,"qualification":if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
