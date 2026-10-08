//! Full-game RAV Sunhome authority for its colorless mana and Double Strike abilities.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaAbilityOutput, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, TargetRequirement, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SUNHOME-FORTRESS";
const CARD_NAME: &str = "Sunhome, Fortress of the Legion";
const ABILITY_ID: &str = "grant-target-double-strike";
const MANA_ABILITY_ID: &str = "produce-colorless";
const TARGET_ID: &str = "RAV-SKYKNIGHT-LEGIONNAIRE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sunhome_fortress_of_the_legion.rs";
const MAX_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sunhome_fortress_of_the_legion.rs"]
mod candidate_sunhome;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Sunhome PvP result: {error}");
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
            eprintln!("Sunhome PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_implementation()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.sunhome != replay.sunhome
        || captured.target != replay.target
        || captured.colorless_mana_source_tapped != replay.colorless_mana_source_tapped
        || captured.ability_source_tapped != replay.ability_source_tapped
        || captured.double_strike_seen != replay.double_strike_seen
        || captured.double_strike_at_attack != replay.double_strike_at_attack
    {
        return Err("Sunhome full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "colorless_output_mutation_detected": mutation_fails(seed, Mutation::WrongManaColor,
            "Sunhome colorless mana ability receipt missing")?,
        "activated_cost_mutation_detected": mutation_fails(seed, Mutation::WrongAbilityCost,
            "Sunhome did not activate the exact {2}{R}{W} ability")?,
        "tap_cost_mutation_detected": mutation_fails(seed, Mutation::NoTapCost,
            "Sunhome activation did not tap its source")?,
        "double_strike_effect_mutation_detected": mutation_fails(seed, Mutation::NoDoubleStrike,
            "the targeted creature did not gain Double Strike")?,
        "sealed_mutation_authority": false
    });
    for key in [
        "colorless_output_mutation_detected",
        "activated_cost_mutation_detected",
        "tap_cost_mutation_detected",
        "double_strike_effect_mutation_detected",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Sunhome semantic mutation escaped the trace oracle: {key}"
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
        "card": {"collector_number": 282, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: color and colored mana costs",
                "106.1 and 106.4: mana production and spell or ability payment",
                "117: priority and timing",
                "305.1 and 305.6: playing lands and land subtypes",
                "602.1-602.2: activating abilities and paying activation costs",
                "608.2-608.3: resolving the activated ability",
                "611.2a and 613: until-end-of-turn continuous effects",
                "508 and 510: declaring attackers and combat damage",
                "702.4: Double Strike"
            ],
            "source_contract": "RAV #282's colorless mana ability and {2}{R}{W}, tap: target creature gains Double Strike until end of turn",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "reference_pilot_pass; independent review and candidate trial remain",
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

fn validate_candidate_implementation() -> Result<(), String> {
    let definition = candidate_sunhome::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sunhome definition missing from the RAV registry")?;
    if definition != registered_definition
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(0)
        || !definition.colors.is_empty()
        || definition.mana_colors != BTreeSet::from([Color::Colorless])
        || definition.card_types != BTreeSet::from([CardType::Land])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err(
            "Sunhome printed identity or land characteristics differ from RAV #282".to_owned(),
        );
    }
    let mana = candidate_sunhome::colorless_mana_ability();
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sunhome colorless mana binding is missing")?;
    if mana != registered_mana.ability
        || mana.id != MANA_ABILITY_ID
        || !mana.tap_cost
        || mana.output != ManaAbilityOutput::Fixed(Color::Colorless)
        || mana.amount != 1
        || mana.life_payment.is_some()
        || mana.controller_damage.is_some()
    {
        return Err("Sunhome must tap to add one colorless mana".to_owned());
    }
    let ability = candidate_sunhome::double_strike_ability();
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sunhome Double Strike binding is missing")?;
    if ability != registered_ability.ability
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(2, [Color::Red, Color::White])
        || !ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || ability.effects
            != [Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::DoubleStrike,
            }]
    {
        return Err("Sunhome ability must be {2}{R}{W}, tap: target creature gains Double Strike until end of turn".to_owned());
    }
    Ok(())
}

/// Allows only a few pure imports and three crate-visible functions returning direct literals.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sunhome source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sunhome source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 5 {
        return Err("Sunhome source has unsupported module attributes or item count".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedManaAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaAbilityOutput"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Sunhome source contains an unsupported import".to_owned());
                };
                actual_imports.extend(paths);
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attr| {
                        is_doc_attribute(attr)
                            || attr.path().is_ident("must_use")
                                && matches!(attr.meta, Meta::Path(_))
                    })
                {
                    return Err(format!(
                        "Sunhome function {name} is not a pure literal constructor"
                    ));
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(format!(
                        "Sunhome function {name} must return one direct struct literal"
                    ));
                };
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "colorless_mana_ability" => "ActivatedManaAbility",
                    "double_strike_ability" => "ActivatedAbility",
                    _ => return Err(format!("unexpected Sunhome function {name}")),
                };
                if !path_is(&literal.path, &[expected_type])
                    || literal.rest.is_some()
                    || literal
                        .fields
                        .iter()
                        .any(|field| !safe_value_expr(&field.expr))
                {
                    return Err(format!(
                        "Sunhome {name} literal contains unsupported behavior"
                    ));
                }
            }
            _ => return Err("Sunhome source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "colorless_mana_ability".to_owned(),
                "double_strike_ability".to_owned(),
            ])
    {
        return Err("Sunhome imports or candidate API differ from the task contract".to_owned());
    }
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}

fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(segment.arguments, PathArguments::None)
        })
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<BTreeSet<Vec<String>>> {
    fn walk(
        tree: &UseTree,
        prefix: &mut Vec<String>,
        paths: &mut BTreeSet<Vec<String>>,
    ) -> Option<()> {
        match tree {
            UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                walk(&path.tree, prefix, paths)?;
                prefix.pop();
            }
            UseTree::Name(name) => {
                let mut full = prefix.clone();
                full.push(name.ident.to_string());
                paths.insert(full);
            }
            UseTree::Group(group) => {
                for tree in &group.items {
                    walk(tree, prefix, paths)?;
                }
            }
            UseTree::Rename(_) | UseTree::Glob(_) => return None,
        }
        Some(())
    }
    let mut paths = BTreeSet::new();
    walk(&item.tree, &mut Vec::new(), &mut paths)?;
    Some(paths)
}

fn safe_value_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => {
            path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path
                    .path
                    .segments
                    .iter()
                    .all(|segment| matches!(segment.arguments, PathArguments::None))
        }
        Expr::Lit(literal) => matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Array(array) => array.elems.iter().all(safe_value_expr),
        Expr::Reference(reference) => {
            reference.mutability.is_none() && safe_value_expr(&reference.expr)
        }
        Expr::Struct(value) => {
            value.rest.is_none()
                && value
                    .fields
                    .iter()
                    .all(|field| safe_value_expr(&field.expr))
        }
        Expr::Call(call) => {
            let Expr::Path(path) = call.func.as_ref() else {
                return false;
            };
            (path_is(&path.path, &["BTreeSet", "new"])
                || path_is(&path.path, &["BTreeSet", "from"])
                || path_is(&path.path, &["ManaCost", "new"])
                || path_is(&path.path, &["ManaCost", "with_colors"])
                || path_is(&path.path, &["ManaAbilityOutput", "Fixed"]))
                && call.args.iter().all(safe_value_expr)
        }
        Expr::Macro(value) => {
            if !value.mac.path.is_ident("vec") || !value.attrs.is_empty() {
                return false;
            }
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            parser
                .parse2(value.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(safe_value_expr))
        }
        Expr::Paren(value) => safe_value_expr(&value.expr),
        Expr::Group(value) => safe_value_expr(&value.expr),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaColor,
    WrongAbilityCost,
    NoTapCost,
    NoDoubleStrike,
}

fn mutation_fails(seed: u64, mutation: Mutation, expected: &str) -> Result<bool, String> {
    let observed = match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).err(),
        Err(error) => Some(error),
    };
    if let Some(error) = observed.as_deref()
        && error.contains(expected)
    {
        return Ok(true);
    }
    Err(format!(
        "mutation {mutation:?} was not detected by expected fragment `{expected}`; observed {observed:?}"
    ))
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    sunhome: Option<ObjectId>,
    target: Option<ObjectId>,
    colorless_mana_source_tapped: Option<bool>,
    ability_source_tapped: Option<bool>,
    target_has_double_strike: bool,
    double_strike_seen: bool,
    double_strike_at_attack: bool,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SUNHOME_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SUNHOME_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no complete Sunhome colorless-cast and Double Strike match found for seeds 0..={MAX_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sunhome missing from definitions")? = candidate_sunhome::definition();
    let mut mana = rav_mana_ability_bindings();
    mana.iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sunhome mana binding missing")?
        .ability = candidate_sunhome::colorless_mana_ability();
    if mutation == Mutation::WrongManaColor {
        mana.iter_mut()
            .find(|binding| binding.card_definition == CARD_ID)
            .expect("checked above")
            .ability
            .output = ManaAbilityOutput::Fixed(Color::Green);
    }
    let mut activated = rav_activated_ability_bindings();
    let ability = &mut activated
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Sunhome activated binding missing")?
        .ability;
    *ability = candidate_sunhome::double_strike_ability();
    match mutation {
        Mutation::WrongAbilityCost => {
            ability.mana_cost = ManaCost::with_colors(3, [Color::Red, Color::White])
        }
        Mutation::NoTapCost => ability.tap_cost = false,
        Mutation::NoDoubleStrike => {
            ability.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::FirstStrike,
            }]
        }
        _ => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        mana,
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated,
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

    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 17,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 17,
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
    for (label, deck) in [("Sunhome pilot", &deck0), ("pass opponent", &deck1)] {
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
        max_turns: 80,
        max_policy_moves: 12_000,
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
    let ability_total = if mutation == Mutation::WrongAbilityCost {
        5
    } else {
        4
    };
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SunhomePolicy::new(PlayerId(0), ability_total)),
        Box::new(PassPolicy {
            player: PlayerId(1),
        }),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        sunhome: None,
        target: None,
        colorless_mana_source_tapped: None,
        ability_source_tapped: None,
        target_has_double_strike: false,
        double_strike_seen: false,
        double_strike_at_attack: false,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Sunhome match exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Sunhome policy seat missing")?;
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
        let was_sunhome_mana = matches!(&action, PolicyAction::ActivateBoundManaAbility { activation }
            if Some(activation.source) == captured.sunhome && activation.ability_id == MANA_ABILITY_ID);
        if let PolicyAction::DeclareAttackers { attackers } = &action
            && attackers.iter().any(|attacker| {
                view.own_battlefield.iter().any(|card| {
                    card.id == *attacker
                        && card.definition == Some(TARGET_ID)
                        && card.keywords.contains(&Keyword::DoubleStrike)
                })
            })
        {
            captured.double_strike_at_attack = true;
        }
        let ability_action = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some((activation.source, activation.targets.first().copied()))
            }
            _ => None,
        };
        if let Some((source, Some(Target::Permanent(target)))) = ability_action {
            captured.sunhome.get_or_insert(source);
            captured.target.get_or_insert(target);
        }
        let before = game.event_log.len();
        let action_label = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Sunhome action rejected at turn {} {:?} policy {} view {} active {} with {action_label}: {error}",
                    view.turn, view.step, player.0, view.player.0, view.active_player.0
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if was_sunhome_mana {
            captured.colorless_mana_source_tapped = Some(
                game.object(
                    captured
                        .sunhome
                        .ok_or("Sunhome source was not identified")?,
                )
                .is_ok_and(|object| object.tapped),
            );
        }
        if let Some((source, _)) = ability_action {
            captured.ability_source_tapped =
                Some(game.object(source).is_ok_and(|object| object.tapped));
        }
        observe_live_state(&game, &mut captured)?;
        for event in &game.event_log[before..] {
            if matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *ability == ABILITY_ID && Some(*source) == captured.sunhome)
            {
                captured.double_strike_seen = true;
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    Ok(captured)
}

fn observe_live_state(game: &Game, captured: &mut CapturedMatch) -> Result<(), String> {
    let view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    for card in &view.own_battlefield {
        if card.definition == Some(CARD_ID) {
            captured.sunhome.get_or_insert(card.id);
        }
        if card.definition == Some(TARGET_ID) {
            captured.target.get_or_insert(card.id);
            captured.target_has_double_strike |= card.keywords.contains(&Keyword::DoubleStrike);
        }
    }
    Ok(())
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Sunhome PvP match did not reach a clean seat-zero terminal win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Sunhome trace contains rejected moves".to_owned());
    }
    let source = captured
        .sunhome
        .ok_or("Sunhome source did not enter the battlefield")?;
    let target = captured.target.ok_or("Skyknight target was not cast")?;
    if captured.colorless_mana_source_tapped != Some(true) {
        return Err("Sunhome did not produce colorless mana for the real creature cast".to_owned());
    }
    if captured.ability_source_tapped != Some(true) {
        return Err("Sunhome activation did not tap its source".to_owned());
    }
    if !captured.double_strike_seen
        || !captured.target_has_double_strike
        || !captured.double_strike_at_attack
    {
        return Err("the targeted creature did not gain Double Strike before attacking".to_owned());
    }
    let mana_activation = event_index(&captured.events, |event| {
        matches!(event,
        GameEvent::BoundManaAbilityActivated { player: PlayerId(0), source: mana_source, ability: MANA_ABILITY_ID,
            color: Color::Colorless, amount: 1, tapped: true, .. } if *mana_source == source)
    });
    let mana_activation =
        mana_activation.ok_or("Sunhome colorless mana ability receipt missing")?;
    let creature_paid = event_index(&captured.events, |event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == target && colors.len() == 3 && colors.contains(&Color::Red)
                && colors.contains(&Color::White) && colors.contains(&Color::Colorless))
    })
    .ok_or("Skyknight spell did not spend red, white, and Sunhome's colorless mana")?;
    let creature_cast = event_index_after(&captured.events, creature_paid, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == target)
    })
    .ok_or("target creature cast receipt missing")?;
    let creature_resolved = event_index_after(&captured.events, creature_cast, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == target)
    })
    .ok_or("target creature resolution missing")?;
    let entered = event_index_after(&captured.events, creature_resolved, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target)
    })
    .ok_or("target creature battlefield entry missing")?;
    let activation_payment = event_index_after(&captured.events, entered, |event| {
        matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source,
            ability: ABILITY_ID, mana_cost } if *paid_source == source
                && *mana_cost == ManaCost::with_colors(2, [Color::Red, Color::White]))
    })
    .ok_or("Sunhome did not activate the exact {2}{R}{W} ability")?;
    let activation = event_index_after(&captured.events, activation_payment, |event| {
        matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source: active_source,
            definition: CARD_ID, ability: ABILITY_ID, .. } if *active_source == source)
    })
    .ok_or("Sunhome activated ability receipt missing")?;
    let resolution = event_index_after(&captured.events, activation, |event| {
        matches!(event,
        GameEvent::AbilityResolved { source: resolved_source, ability: ABILITY_ID, .. }
            if *resolved_source == source)
    })
    .ok_or("Sunhome ability did not resolve")?;
    let target_attack = event_index_after(&captured.events, resolution, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target)))
        .ok_or("Double Strike target did not attack after resolution")?;
    let first_strike_damage = event_index_after(&captured.events, target_attack, |event| {
        matches!(event,
        GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount: 2 }
            if *damage_source == target)
    })
    .ok_or("first Double Strike damage packet missing")?;
    let second_strike_damage = event_index_after(&captured.events, first_strike_damage, |event| {
        matches!(event,
        GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount: 2 }
            if *damage_source == target)
    })
    .ok_or("regular Double Strike damage packet missing")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded receipt missing")?;
    if !(mana_activation < creature_paid
        && creature_paid < creature_cast
        && creature_cast < creature_resolved
        && creature_resolved < entered
        && entered < activation_payment
        && activation_payment < activation
        && activation < resolution
        && resolution < target_attack
        && target_attack < first_strike_damage
        && first_strike_damage < second_strike_damage
        && second_strike_damage < ended)
    {
        return Err("Sunhome mana, creature cast, activated effect, double-strike combat, and terminal order differs".to_owned());
    }
    let projected = project_events(&captured.events);
    let relevant = project_relevant(&projected, source, target);
    Ok(json!({
        "id": "rav-sunhome-double-strike-targeted-activation",
        "seed": seed,
        "decks": ["rav_sunhome_double_strike", "rav_sunhome_pass_opponent"],
        "policies": ["rav.sunhome-double-strike-pilot.v1", "rav.sunhome-pass-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "sunhome_object": source.0,
        "target_creature_object": target.0,
        "sunhome_colorless_mana": {"ability": MANA_ABILITY_ID, "color": "Colorless", "amount": 1, "source_tapped": captured.colorless_mana_source_tapped},
        "target_cast_payment": {"required": {"generic_symbols": 1, "red_symbols": 1, "white_symbols": 1, "total_symbols": 3}, "spent": {"colorless": 1, "red": 1, "white": 1}},
        "activated_ability_payment": {"required": "{2}{R}{W}", "source_tapped": captured.ability_source_tapped},
        "target_gained_double_strike": captured.double_strike_seen,
        "target_had_double_strike_at_attack": captured.double_strike_at_attack,
        "double_strike_damage_packets": 2,
        "ordered_event_sequence": relevant.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        "events": relevant,
        "passed": true
    }))
}

fn project_relevant(
    events: &[ProjectedEvent],
    sunhome: ObjectId,
    target: ObjectId,
) -> Vec<ProjectedEvent> {
    events
        .iter()
        .filter(|event| {
            event.kind == "GameEnded"
                || event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == sunhome.0 || object.0 == target.0)
        })
        .cloned()
        .collect()
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
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

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut bytes = String::new();
    for event in events {
        let _ = writeln!(bytes, "{event:?}");
    }
    let hash = bytes
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

struct SunhomePolicy {
    player: PlayerId,
    target_cast: bool,
    ability_used: bool,
    ability_mana_total: u8,
}

impl SunhomePolicy {
    fn new(player: PlayerId, ability_mana_total: u8) -> Self {
        Self {
            player,
            target_cast: false,
            ability_used: false,
            ability_mana_total,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let has_sunhome = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        view.hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && !has_sunhome)
            .or_else(|| {
                view.hand.iter().find(|card| {
                    card.card_types.contains(&CardType::Land) && card.definition != Some(CARD_ID)
                })
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn basic_mana(&self, view: &GameView, wanted: Option<Color>) -> Option<PolicyAction> {
        let choices = match wanted {
            Some(Color::Red) => vec![("RAV-MOUNTAIN", Color::Red)],
            Some(Color::White) => vec![("RAV-PLAINS", Color::White)],
            Some(Color::Green) => vec![("RAV-FOREST", Color::Green)],
            _ => vec![
                ("RAV-FOREST", Color::Green),
                ("RAV-MOUNTAIN", Color::Red),
                ("RAV-PLAINS", Color::White),
            ],
        };
        choices.into_iter().find_map(|(id, color)| {
            view.own_battlefield
                .iter()
                .find(|card| card.definition == Some(id) && !card.tapped)
                .map(|land| PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                })
        })
    }

    fn sunhome_mana(&self, view: &GameView) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && !card.tapped)?;
        Some(PolicyAction::ActivateBoundManaAbility {
            activation: cardbench_magic_engine::ManaAbilityActivation {
                source: land.id,
                ability_id: MANA_ABILITY_ID,
                chosen_color: None,
            },
        })
    }

    fn cast_target(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.target_cast {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(TARGET_ID))?;
        let cost = ManaCost::with_colors(1, [Color::Red, Color::White]);
        if let Some(mana_selection) = payment(view, &cost) {
            self.target_cast = true;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        if view.mana_pool.amount(Color::Red) == 0 {
            return self.basic_mana(view, Some(Color::Red));
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.basic_mana(view, Some(Color::White));
        }
        if view.mana_pool.total() < 3 && view.mana_pool.amount(Color::Colorless) == 0 {
            if let Some(action) = self.sunhome_mana(view) {
                return Some(action);
            }
        }
        self.basic_mana(view, None)
    }

    fn activate_double_strike(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.ability_used {
            return None;
        }
        let target = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(TARGET_ID) && card.can_attack)?;
        let source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && !card.tapped)?;
        if view.mana_pool.amount(Color::Red) == 0 {
            return self.basic_mana(view, Some(Color::Red));
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.basic_mana(view, Some(Color::White));
        }
        if view.mana_pool.total() < self.ability_mana_total {
            return self.basic_mana(view, None);
        }
        self.ability_used = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: source.id,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(target.id)],
            },
        })
    }
}

impl CodePolicy for SunhomePolicy {
    fn id(&self) -> &'static str {
        "rav.sunhome-double-strike-pilot.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
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
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step != Step::PrecombatMain || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if let Some(action) = self.cast_target(view) {
            return action;
        }
        if let Some(action) = self.activate_double_strike(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct PassPolicy {
    player: PlayerId,
}
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.sunhome-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
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
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status":if passed {"passed"} else {"not_qualified"},"credited":false,
        "qualification":if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
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
