//! Full-game diagnostic authority for RAV Twilight Drover #33.
//!
//! The trace creates three Saprolings, then returns one token and an opposing
//! nontoken creature simultaneously. Exactly the token departure must trigger
//! Drover. Its counter is spent as the activation cost for two typed white
//! Flying Spirit tokens. The game then continues to a terminal PvP result.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, ActivatedAbilityBinding, ActivatedAbilityCostBinding,
    ActivatedCounterCost, ActivatedCounterCostTarget, CardDefinition, CardType, CastRequest, Color,
    CounterKind, CreatureSubtype, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    GeneralizedAbilityActivation, GeneralizedActivatedAbilityCost, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TokenSpec,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-TWILIGHT-DROVER";
const CARD_NAME: &str = "Twilight Drover";
const TRIGGER_ID: &str = "creature-token-leaves-plus-one-counter";
const ACTIVATION_ID: &str = "create-two-flying-spirits";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/twilight_drover.rs";
const SAPROLING_ID: &str = "RAV-SCATTER-THE-SEEDS";
const PEEL_ID: &str = "RAV-PEEL-FROM-REALITY";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;
const MAX_TURNS: u32 = 48;
const MAX_POLICY_MOVES: u32 = 7_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/twilight_drover.rs"]
mod candidate_twilight_drover;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPower,
    BroadCreatureTrigger,
    OptionalTrigger,
    CounterCostAmount,
    OneSpirit,
    WrongActivationCost,
    MissingTrigger,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    drover: Option<ObjectId>,
    scatter: Option<ObjectId>,
    peel: Option<ObjectId>,
    token_target: Option<ObjectId>,
    nontoken_target: Option<ObjectId>,
    drover_paid: Vec<Color>,
    scatter_paid: Vec<Color>,
    peel_paid: Vec<Color>,
    spirits: Vec<Value>,
    drover_pt: Option<(i32, i32)>,
}

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Twilight Drover result: {error}");
                return ExitCode::FAILURE;
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
            eprintln!("Twilight Drover PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // This source and AST guard is intentionally the first candidate-dependent
    // operation. No submitted factory is called until the source is accepted.
    validate_candidate_source()?;
    let injected_side_effect_fixture = injected_side_effect_fixture_rejected();
    if !injected_side_effect_fixture {
        return Err("literal-only candidate guard accepted an injected side effect".to_owned());
    }
    validate_candidate_values(
        candidate_twilight_drover::definition(),
        candidate_twilight_drover::triggered_ability(),
        candidate_twilight_drover::activated_ability(),
        candidate_twilight_drover::activated_ability_cost(),
    )?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Twilight Drover full-game replay differed".to_owned());
    }

    let mutations = json!({
        "wrong_body_rejected_by_trace": mutation_detected(seed, Mutation::WrongPower),
        "nontoken_departure_trigger_rejected_by_trace": mutation_detected(seed, Mutation::BroadCreatureTrigger),
        "optional_trigger_rejected_by_trace": mutation_detected(seed, Mutation::OptionalTrigger),
        "wrong_counter_cost_rejected_by_trace": mutation_detected(seed, Mutation::CounterCostAmount),
        "one_spirit_rejected_by_trace": mutation_detected(seed, Mutation::OneSpirit),
        "wrong_activation_cost_rejected_by_trace": mutation_detected(seed, Mutation::WrongActivationCost),
        "missing_trigger_rejected_by_trace": mutation_detected(seed, Mutation::MissingTrigger),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_body_rejected_by_trace",
        "nontoken_departure_trigger_rejected_by_trace",
        "optional_trigger_rejected_by_trace",
        "wrong_counter_cost_rejected_by_trace",
        "one_spirit_rejected_by_trace",
        "wrong_activation_cost_rejected_by_trace",
        "missing_trigger_rejected_by_trace",
    ] {
        if mutations[key] != true {
            return Err(format!("Twilight Drover semantic mutant escaped: {key}"));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":33,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "111 Tokens and token characteristics",
                "117 Priority and timing",
                "302.1-302.6 Creature permanents and summoning sickness",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities and costs",
                "603.2 Trigger events and token departures",
                "608.2 Resolving triggered and activated abilities",
                "701.26 Returning a permanent to its owner's hand"
            ],
            "source_contract":"RAV printed Twilight Drover #33 is a white {2}{W} 1/1 Spirit. Whenever a creature token leaves the battlefield, a mandatory trigger puts one +1/+1 counter on it. Its {2}{W} activation removes one +1/+1 counter from itself and creates two 1/1 white Spirit creature tokens with flying. Printed-era wording uses 'leaves play' and 'into play'; current Oracle modernizes those terms without changing these modeled operations.",
            "source_references":["https://www.mtg.wtf/card/rav/33/Twilight-Drover","https://www.lastdodo.com/en/items/829511-twilight-drover"],
            "creature_subtype_boundary":"CardDefinition does not model ordinary creature subtypes. The printed Spirit identity is retained in this task source note and is not asserted for the nontoken candidate body. The two created tokens are fully typed Spirit tokens.",
            "oracle_reconciliation":"Current Oracle wording is semantically aligned for this task: creature tokens leaving the battlefield trigger the mandatory counter, and the activation creates two white flying Spirit tokens after removing a counter. No later wording is used to add unsupported nontoken departure behavior.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,
        "candidate_source_validation":{
            "literal_ast_contract_matched":true,
            "guard_before_candidate_calls":true,
            "parsed_side_effect_fixture_rejected":injected_side_effect_fixture,
            "compile_valid_fixture_claimed":false
        },
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
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
    {
        return Err("RAV catalog denominator changed".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_values(
    definition: CardDefinition,
    trigger: TriggeredAbilityBinding,
    activation: ActivatedAbilityBinding,
    cost: ActivatedAbilityCostBinding,
) -> Result<(), String> {
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "creature-token-leaves-battlefield-plus-one-counter",
                "activated-remove-counter-create-two-flying-spirits",
            ]
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Twilight Drover printed definition differs".to_owned());
    }
    if trigger.card_definition != CARD_ID
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition != TriggerCondition::AnotherCreatureTokenLeavesBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::AddPlusOneCounterToSource]
    {
        return Err("Twilight Drover mandatory token-leaves trigger differs".to_owned());
    }
    if activation.card_definition != CARD_ID
        || activation.ability.id != ACTIVATION_ID
        || activation.ability.mana_cost != ManaCost::with_colors(2, [Color::White])
        || activation.ability.tap_cost
        || activation.ability.sorcery_speed
        || activation.ability.additional_tap_creatures != 0
        || activation.ability.sacrifice_source
        || activation.ability.sacrifice_creatures != 0
        || activation.ability.sacrifice_lands != 0
        || activation.ability.discard_cards != 0
        || !activation.ability.targets.is_empty()
        || activation.ability.effects
            != [Effect::CreateToken {
                token: TokenSpec::white_spirit(),
                count: 2,
            }]
    {
        return Err("Twilight Drover two-Spirit activation differs".to_owned());
    }
    let expected_cost = GeneralizedActivatedAbilityCost {
        counter_removals: vec![ActivatedCounterCost {
            target: ActivatedCounterCostTarget::Source,
            counter: CounterKind::PlusOnePlusOne,
            amount: 1,
        }],
        ..GeneralizedActivatedAbilityCost::default()
    };
    if cost.card_definition != CARD_ID
        || cost.ability_id != ACTIVATION_ID
        || cost.cost != expected_cost
    {
        return Err("Twilight Drover activation must remove one source +1/+1 counter".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Twilight Drover source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Twilight Drover source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Twilight Drover has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let paths = flattened_use_paths(item)
                    .ok_or("Twilight Drover source has an unsupported import")?;
                imports.extend(paths);
            }
            Item::Fn(function) => {
                if !function.attrs.iter().all(|attr| {
                    (attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
                        || attr.path().is_ident("must_use")
                }) || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || !function.sig.inputs.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                {
                    return Err(
                        "Twilight Drover functions must be simple crate-visible literal factories"
                            .to_owned(),
                    );
                }
                let name = function.sig.ident.to_string();
                if functions.contains_key(&name) {
                    return Err("Twilight Drover source repeats a factory".to_owned());
                }
                let Some(Stmt::Expr(Expr::Struct(value), None)) = function.block.stmts.first()
                else {
                    return Err(
                        "Twilight Drover factory must return one direct struct literal".to_owned(),
                    );
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Twilight Drover source contains an unsupported executable expression"
                            .to_owned(),
                    );
                }
                functions.insert(name, function);
            }
            _ => return Err("Twilight Drover source contains executable top-level code".to_owned()),
        }
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "ActivatedAbilityCostBinding"],
        vec!["cardbench_magic_engine", "ActivatedCounterCost"],
        vec!["cardbench_magic_engine", "ActivatedCounterCostTarget"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "CounterKind"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "GeneralizedActivatedAbilityCost"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TokenSpec"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if imports != expected_imports
        || functions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([
                "definition",
                "triggered_ability",
                "activated_ability",
                "activated_ability_cost",
            ])
    {
        return Err("Twilight Drover module imports or factory set differs".to_owned());
    }
    Ok(())
}

fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
                && value.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a,b] if (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "ManaCost" && (b == "new" || b == "with_colors")) || (a == "GeneralizedActivatedAbilityCost" && b == "default") || (a == "TokenSpec" && b == "white_spirit"))
                        || names.as_slice() == ["Some"]
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            parser
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn injected_side_effect_fixture_rejected() -> bool {
    let fixture = r#"
        use std::collections::BTreeSet;
        use cardbench_magic_engine::CardDefinition;
        pub(crate) fn definition() -> CardDefinition {
            { let _ = std::process::Command::new("touch").arg("/tmp/should-not-run").status(); CardDefinition { id: "RAV-TWILIGHT-DROVER", name: "Twilight Drover", set_code: "RAV", mana_cost: cardbench_magic_engine::ManaCost::new(0), colors: BTreeSet::new(), mana_colors: BTreeSet::new(), card_types: BTreeSet::new(), is_basic_land: false, supported_rules: &[], power: None, toughness: None, keywords: vec![], effects: vec![] } }
        }
    "#;
    syn::parse_file(fixture).is_ok() && validate_candidate_source_text(fixture).is_err()
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
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

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_TWILIGHT_DROVER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_TWILIGHT_DROVER_SEED must be unsigned".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 1..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no deterministic Twilight Drover trace found in seeds 1..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SAPROLING_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: PEEL_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 12,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
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
    for (seat, deck) in [(0, &p0), (1, &p1)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!(
                "seat {seat} constructed list contains {size} cards, expected 60"
            ));
        }
    }

    let mut definition = candidate_twilight_drover::definition();
    let mut trigger = candidate_twilight_drover::triggered_ability();
    let mut activation = candidate_twilight_drover::activated_ability();
    let mut cost = candidate_twilight_drover::activated_ability_cost();
    match mutation {
        Mutation::None => {}
        Mutation::WrongPower => definition.power = Some(2),
        Mutation::BroadCreatureTrigger => {
            trigger.ability.condition = TriggerCondition::AnotherCreatureLeavesBattlefield
        }
        Mutation::OptionalTrigger => trigger.ability.optional = true,
        Mutation::CounterCostAmount => cost.cost.counter_removals[0].amount = 2,
        Mutation::OneSpirit => {
            activation.ability.effects = vec![Effect::CreateToken {
                token: TokenSpec::white_spirit(),
                count: 1,
            }]
        }
        Mutation::WrongActivationCost => {
            activation.ability.mana_cost = ManaCost::with_colors(3, [Color::White])
        }
        Mutation::MissingTrigger => {}
    }

    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Twilight Drover definition missing from RAV registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| binding.card_definition != CARD_ID);
    abilities.push(activation);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    triggers.extend(attachment_triggers());
    if mutation != Mutation::MissingTrigger {
        triggers.push(trigger);
    }

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
    let mut costs = rav_generalized_activated_ability_cost_bindings();
    costs.retain(|binding| binding.card_definition != CARD_ID);
    let mut candidate_cost = cost;
    if mutation == Mutation::CounterCostAmount {
        candidate_cost.cost.counter_removals[0].amount = 2;
    }
    costs.push(candidate_cost);
    game.register_generalized_activated_ability_cost_bindings(costs)
        .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;

    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DroverPolicy::new(PlayerId(0))),
        Box::new(WatchwolfPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut peel_targets = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_POLICY_MOVES {
            return Err(format!(
                "Twilight Drover match exceeded its bound on turn {}",
                game.turn
            ));
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
        attempted += 1;
        let event_cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("policy move rejected on turn {}: {error}", view.turn))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        for event in &game.event_log[event_cursor..] {
            if let GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == PEEL_ID)
                && let Some(stack_spell) = game.stack.iter().find(|item| item.card == *card)
            {
                peel_targets = Some(stack_spell.targets.clone());
            }
        }
    }
    let events = game.event_log.clone();
    let find_cast = |id: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game.card_definition(*card).is_ok_and(|def| def.id == id) => Some(*card),
            _ => None,
        })
    };
    let drover = find_cast(CARD_ID);
    let scatter = find_cast(SAPROLING_ID);
    let peel = find_cast(PEEL_ID);
    let drover_paid = paid_colors(&events, drover);
    let scatter_paid = paid_colors(&events, scatter);
    let peel_paid = paid_colors(&events, peel);
    let targets = peel_targets.unwrap_or_default();
    let (token_target, nontoken_target) = match targets.as_slice() {
        [Target::Permanent(first), Target::Permanent(second)] => (Some(*first), Some(*second)),
        _ => (None, None),
    };
    let activation_started = events.iter().position(|event| matches!(event, GameEvent::AbilityActivated { source, definition: CARD_ID, ability: ACTIVATION_ID, .. } if Some(*source) == drover));
    let spirits = events.iter().enumerate().filter_map(|(index, event)| {
        let GameEvent::TokenCreated { player: PlayerId(0), token } = event else { return None; };
        if activation_started.is_none_or(|started| index <= started) { return None; }
        let chars = game.characteristics(*token).ok()?;
        (chars.creature_subtypes.contains(&CreatureSubtype::Spirit)).then(|| json!({
            "object":token.0,
            "colors":chars.colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "subtypes":chars.creature_subtypes.iter().map(|subtype| format!("{subtype:?}")).collect::<Vec<_>>(),
            "power":chars.power,
            "toughness":chars.toughness,
            "flying":chars.keywords.contains(&Keyword::Flying),
        }))
    }).collect();
    let drover_pt = drover
        .and_then(|source| game.characteristics(source).ok())
        .and_then(|chars| Some((chars.power?, chars.toughness?)));
    Ok(CapturedMatch {
        events,
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        drover,
        scatter,
        peel,
        token_target,
        nontoken_target,
        drover_paid,
        scatter_paid,
        peel_paid,
        spirits,
        drover_pt,
    })
}

fn paid_colors(events: &[GameEvent], game: &Game, card: Option<ObjectId>) -> Vec<Color> {
    card.and_then(|card| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card: paid,
                colors,
            } if *paid == card => Some(colors.clone()),
            _ => None,
        })
    })
    .unwrap_or_else(|| {
        let _ = game;
        Vec::new()
    })
}

struct DroverPolicy {
    player: PlayerId,
    peel_cast: bool,
    counter_expected: bool,
    activated: bool,
    scatter_cast: bool,
}
impl DroverPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            peel_cast: false,
            counter_expected: false,
            activated: false,
            scatter_cast: false,
        }
    }
}
struct WatchwolfPolicy {
    player: PlayerId,
    cast_once: bool,
}
impl WatchwolfPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast_once: false,
        }
    }
}

impl CodePolicy for DroverPolicy {
    fn id(&self) -> &'static str {
        "rav.twilight-drover-full-game.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player || view.priority != self.player {
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
                    .filter(|card| card.can_attack)
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
        if self.peel_cast && !self.activated && view.stack_depth == 0 {
            self.counter_expected = true;
        }
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        let drover = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
            .map(|card| card.id);
        if drover.is_none()
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = ManaCost::with_colors(2, [Color::White]);
            if let Some(selection) = payment_selection(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        if drover.is_some()
            && !self.scatter_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SAPROLING_ID))
        {
            let cost = ManaCost::with_colors(2, [Color::Green]);
            if let Some(selection) = payment_selection(view, &cost) {
                self.scatter_cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        let token = view.own_battlefield.iter().find(|card| {
            card.definition.is_none() && card.card_types.contains(&CardType::Creature)
        });
        let opponent = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID));
        if !self.peel_cast
            && token.is_some()
            && opponent.is_some()
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(PEEL_ID))
        {
            let cost = ManaCost::with_colors(1, [Color::Blue]);
            if let Some(selection) = payment_selection(view, &cost) {
                self.peel_cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![
                            Target::Permanent(token.expect("checked").id),
                            Target::Permanent(opponent.expect("checked").id),
                        ],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        if self.counter_expected
            && !self.activated
            && let Some(source) = drover
        {
            let cost = ManaCost::with_colors(2, [Color::White]);
            if let Some(selection) = payment_selection(view, &cost) {
                self.activated = true;
                return PolicyAction::ActivateAbilityWithGeneralizedCosts {
                    activation: GeneralizedAbilityActivation {
                        activation: AbilityActivation {
                            source,
                            ability_id: ACTIVATION_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                        cost_payment: AbilityCostPayment {
                            counter_sources: vec![source],
                            ..AbilityCostPayment::default()
                        },
                        mana_payment_selection: Some(selection),
                    },
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for WatchwolfPolicy {
    fn id(&self) -> &'static str {
        "rav.twilight-drover-watchwolf-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player || view.priority != self.player {
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
                    .filter(|card| card.can_attack)
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
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.cast_once
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
        {
            let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
            if let Some(selection) = payment_selection(view, &cost) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut pool: BTreeMap<Color, u8> = Color::ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = pool.get_mut(color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let color = Color::ALL
            .into_iter()
            .find(|color| pool.get(color).is_some_and(|amount| *amount > 0))?;
        *pool.get_mut(&color)? -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let mut needed = BTreeMap::<Color, u8>::new();
    for color in &cost.colored {
        *needed.entry(*color).or_default() += 1;
    }
    for (color, required) in &needed {
        if view.mana_pool.amount(*color) < *required {
            return view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.mana_colors.contains(color))
                .map(|land| PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: *color,
                });
        }
    }
    let colored_needed: u8 = needed.values().copied().sum();
    let total_needed = cost.generic.saturating_add(colored_needed);
    if view.mana_pool.total() < total_needed {
        let source = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && !card.mana_colors.is_empty())?;
        let preferred = Color::ALL
            .into_iter()
            .find(|color| {
                needed
                    .get(color)
                    .is_some_and(|required| view.mana_pool.amount(*color) < *required)
            })
            .or_else(|| source.mana_colors.iter().next().copied())?;
        return Some(PolicyAction::ActivateManaAbility {
            land: source.id,
            color: preferred,
        });
    }
    None
}

fn paid_colors(events: &[GameEvent], card: Option<ObjectId>) -> Vec<Color> {
    let Some(card) = card else {
        return Vec::new();
    };
    events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card: paid,
                colors,
            } if *paid == card => Some(colors.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.turns == right.turns
        && left.drover == right.drover
        && left.scatter == right.scatter
        && left.peel == right.peel
        && left.token_target == right.token_target
        && left.nontoken_target == right.nontoken_target
        && left.drover_paid == right.drover_paid
        && left.scatter_paid == right.scatter_paid
        && left.peel_paid == right.peel_paid
        && left.spirits == right.spirits
        && left.drover_pt == right.drover_pt
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err("game did not reach a clean P0 win".to_owned());
    }
    let drover = capture.drover.ok_or("Twilight Drover was not cast")?;
    let scatter = capture.scatter.ok_or("Scatter the Seeds was not cast")?;
    let peel = capture.peel.ok_or("Peel from Reality was not cast")?;
    let token = capture
        .token_target
        .ok_or("Peel did not select the token target")?;
    let nontoken = capture
        .nontoken_target
        .ok_or("Peel did not select the nontoken target")?;
    if capture.drover_paid.len() != 3
        || !capture.drover_paid.contains(&Color::White)
        || capture.scatter_paid.len() != 3
        || !capture.scatter_paid.contains(&Color::Green)
        || capture.peel_paid.len() != 2
        || !capture.peel_paid.contains(&Color::Blue)
    {
        return Err("printed white/green/blue spell costs were not paid".to_owned());
    }
    if capture.drover_pt != Some((1, 1)) {
        return Err("Twilight Drover is not a live printed 1/1".to_owned());
    }
    let triggers: Vec<_> = capture.events.iter().filter(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == drover)).collect();
    let counters: Vec<_> = capture.events.iter().filter(|event| matches!(event, GameEvent::CounterPlaced { card, counter: CounterKind::PlusOnePlusOne, amount: 1, .. } if *card == drover)).collect();
    let token_moved = capture.events.iter().any(
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == token),
    );
    let token_ceased = capture.events.iter().any(|event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token));
    let nontoken_moved = capture.events.iter().any(
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == nontoken),
    );
    let all_tokens_created = capture
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    ..
                }
            )
        })
        .count();
    let token_move_index = capture.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == token)).ok_or("token-to-hand zone change receipt missing")?;
    let nontoken_move_index = capture.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == nontoken)).ok_or("Watchwolf-to-hand zone change receipt missing")?;
    let trigger_index = capture.events.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == drover)).ok_or("token-only trigger did not reach the stack")?;
    let counter_index = capture.events.iter().position(|event| matches!(event, GameEvent::CounterPlaced { card, counter: CounterKind::PlusOnePlusOne, amount: 1, .. } if *card == drover)).ok_or("trigger did not place its source counter")?;
    let activation_index = capture.events.iter().position(|event| matches!(event, GameEvent::AbilityActivated { source, definition: CARD_ID, ability: ACTIVATION_ID, .. } if *source == drover)).ok_or("Drover activation did not reach the stack")?;
    let resolution_index = capture.events.iter().position(|event| matches!(event, GameEvent::AbilityResolved { source, ability: ACTIVATION_ID, .. } if *source == drover)).ok_or("Drover activation did not resolve")?;
    let cost_event_index = capture.events.iter().position(|event| matches!(event, GameEvent::CounterRemovedAsAbilityCost { player: PlayerId(0), source, card, counter: CounterKind::PlusOnePlusOne, amount: 1 } if *source == drover && *card == drover)).ok_or("source counter cost receipt missing")?;
    let counter_removal_index = capture.events.iter().position(|event| matches!(event, GameEvent::CounterRemoved { card, counter: CounterKind::PlusOnePlusOne, amount: 1, .. } if *card == drover)).ok_or("source +1/+1 counter removal state event missing")?;
    let trigger_stack_identity = capture.events.iter().find_map(|event| match event {
        GameEvent::TriggeredAbilityStacked { source, source_incarnation, ability, .. } if *source == drover && *ability == TRIGGER_ID => Some((*source, *source_incarnation)),
        _ => None,
    });
    let trigger_resolution_identity = capture.events.iter().find_map(|event| match event {
        GameEvent::AbilityResolved { source, source_incarnation, ability } if *source == drover && *ability == TRIGGER_ID => Some((*source, *source_incarnation)),
        _ => None,
    });
    let activation_stack_identity = capture.events.iter().find_map(|event| match event {
        GameEvent::AbilityActivated { source, source_incarnation, definition: CARD_ID, ability: ACTIVATION_ID, .. } if *source == drover => Some((*source, *source_incarnation)),
        _ => None,
    });
    let activation_resolution_identity = capture.events.iter().find_map(|event| match event {
        GameEvent::AbilityResolved { source, source_incarnation, ability } if *source == drover && *ability == ACTIVATION_ID => Some((*source, *source_incarnation)),
        _ => None,
    });
    if triggers.len() != 1 || counters.len() != 1 || all_tokens_created != 5 || !token_moved || !token_ceased || !nontoken_moved
        || token_move_index >= trigger_index || nontoken_move_index >= trigger_index
        || trigger_index >= counter_index || counter_index >= cost_event_index || cost_event_index >= counter_removal_index || counter_removal_index >= activation_index || activation_index >= resolution_index
        || trigger_stack_identity.is_none() || trigger_stack_identity != trigger_resolution_identity
        || activation_stack_identity.is_none() || activation_stack_identity != activation_resolution_identity
        || capture.events.iter().filter(|event| matches!(event, GameEvent::AbilityActivated { source, definition: CARD_ID, ability: ACTIVATION_ID, .. } if *source == drover)).count() != 1
        || capture.events.iter().filter(|event| matches!(event, GameEvent::CounterRemovedAsAbilityCost { player: PlayerId(0), source, card, counter: CounterKind::PlusOnePlusOne, amount: 1 } if *source == drover && *card == drover)).count() != 1
        || capture.events.iter().filter(|event| matches!(event, GameEvent::AbilityResolved { source, ability: ACTIVATION_ID, .. } if *source == drover)).count() != 1
    { return Err("token-only trigger, source counter cost, or simultaneous departure trace differs".to_owned()); }
    if capture.spirits.len() != 2
        || capture.spirits.iter().any(|spirit| {
            spirit["colors"] != json!(["White"])
                || spirit["subtypes"] != json!(["Spirit"])
                || spirit["power"] != 1
                || spirit["toughness"] != 1
                || spirit["flying"] != true
        })
    {
        return Err(
            "activated ability did not create exactly two live white 1/1 Flying Spirits".to_owned(),
        );
    }
    let activation_cost_paid = capture.events.iter().any(|event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ACTIVATION_ID, mana_cost } if *source == drover && *mana_cost == ManaCost::with_colors(2, [Color::White])));
    if !activation_cost_paid {
        return Err("activated ability did not pay its printed {2}{W} mana cost".to_owned());
    }
    let projected = project_events(&capture.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})).collect();
    let trigger_count = triggers.len();
    Ok(json!({
        "id":"rav-twilight-drover-token-only-counter-and-two-spirits",
        "passed":true,
        "seed":seed,
        "decks":["twilight_drover_60","watchwolf_60"],
        "terminal":"Winner(PlayerId(0))",
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted_moves,
        "rejected_moves":capture.rejected_moves,
        "twilight_drover_object":drover.0,
        "twilight_drover_characteristics":{"power":capture.drover_pt.map(|pt|pt.0),"toughness":capture.drover_pt.map(|pt|pt.1)},
        "scatter_the_seeds_object":scatter.0,
        "peel_from_reality_object":peel.0,
        "token_departure":{"object":token.0,"returned_to_owner_hand":token_moved,"ceased_after_zone_change":token_ceased},
        "nontoken_departure":{"object":nontoken.0,"definition_id":WATCHWOLF_ID,"returned_to_owner_hand":nontoken_moved},
        "token_only_trigger_count":trigger_count,
        "source_counter_placed_count":counters.len(),
        "counter_cost":{"source":drover.0,"counter":"PlusOnePlusOne","amount":1,"paid_as_cost":true},
        "activation":{"mana_cost":"{2}{W}","source_counter_removed":true,"created_spirits":capture.spirits},
        "mana_paid":{"drover":capture.drover_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"scatter":capture.scatter_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"peel":capture.peel_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "ordered_event_sequence":events.iter().filter_map(|event| event["kind"].as_str().map(str::to_owned)).collect::<Vec<_>>(),
        "events":events
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
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
