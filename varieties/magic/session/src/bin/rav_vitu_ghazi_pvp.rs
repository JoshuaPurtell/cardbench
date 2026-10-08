//! Full-game diagnostic authority for RAV Vitu-Ghazi, the City-Tree.
//!
//! One copy produces colorless mana to pay a generic spell cost, then untaps
//! and uses its stack-backed Saproling ability before a terminal PvP result.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaAbilityActivation,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TokenSpec, Zone,
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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VITU-GHAZI";
const CARD_NAME: &str = "Vitu-Ghazi, the City-Tree";
const ABILITY_ID: &str = "create-green-saproling";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/vitu_ghazi.rs";
const MAX_TRACE_SEED: u64 = 256;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/vitu_ghazi.rs"]
mod candidate_vitu_ghazi;

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
            eprintln!("Vitu-Ghazi PvP authority failed: {error}");
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
    if !same_match(&captured, &replay)
        || project_events(&captured.events) != project_events(&replay.events)
    {
        return Err("Vitu-Ghazi full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "colorless_mana_binding_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongManaColor),
        "saproling_activation_tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::NoActivationTap),
        "saproling_effect_mutation_detected_in_game": mutation_detected(seed, Mutation::NoToken),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["colorless_mana_binding_mutation_detected_in_game"] != true
        || mutation_sensitivity["saproling_activation_tap_cost_mutation_detected_in_game"] != true
        || mutation_sensitivity["saproling_effect_mutation_detected_in_game"] != true
    {
        return Err("a Vitu-Ghazi semantic mutation escaped the full-game trace".to_owned());
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
        "card": {"collector_number": 285, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "305.1-305.2: playing a land during a main phase with priority and an empty stack",
                "106.1 and 106.4: colorless mana production and mana payment",
                "605.1a and 605.3: mana abilities and tap costs",
                "117.1b and 117.3c: priority and stack resolution",
                "602.1-602.2: activating a stack-using ability and paying costs",
                "111.3: token characteristics",
                "508 and 510: combat and damage"
            ],
            "source_contract": "RAV Vitu-Ghazi, the City-Tree is a colorless land with a tap-for-colorless mana ability and a {2}{G}{W}, tap activated Saproling ability",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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
        return Err("RAV catalog identity or 291/306 denominator changed".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Vitu-Ghazi source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Vitu-Ghazi source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Vitu-Ghazi module contains an unsupported attribute".to_owned());
    }
    let mut functions = BTreeSet::new();
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedManaAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaAbilityOutput"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TokenSpec"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut actual_imports)?;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if ![
                    "definition",
                    "colorless_mana_ability",
                    "create_saproling_ability",
                ]
                .contains(&name.as_str())
                    || !functions.insert(name)
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| is_doc_attribute(attr) || attr.path().is_ident("must_use"))
                    || !matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"))
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !function.sig.inputs.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none())
                {
                    return Err(
                        "Vitu-Ghazi module must contain only three direct literal functions"
                            .to_owned(),
                    );
                }
            }
            _ => return Err("Vitu-Ghazi module contains executable top-level code".to_owned()),
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "colorless_mana_ability".to_owned(),
            "create_saproling_ability".to_owned(),
        ])
        || actual_imports != expected_imports
    {
        return Err("Vitu-Ghazi imports or required candidate-owned functions differ".to_owned());
    }
    // All source operations used in these literal constructors are pure value
    // constructors. Reject nested closures, blocks, assignments, and methods.
    for function in file.items.iter().filter_map(|item| {
        if let Item::Fn(function) = item {
            Some(function)
        } else {
            None
        }
    }) {
        let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
            unreachable!()
        };
        if !safe_path(&value.path)
            || !value
                .fields
                .iter()
                .all(|field| safe_value_expr(&field.expr))
        {
            return Err(
                "Vitu-Ghazi source contains a nonliteral or unsupported field expression"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    out: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, out)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            out.insert(path);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, out)?;
            }
            Ok(())
        }
        UseTree::Glob(_) | UseTree::Rename(_) => {
            Err("Vitu-Ghazi imports cannot use aliases or globs".to_owned())
        }
    }
}
fn safe_path(path: &syn::Path) -> bool {
    path.leading_colon.is_none()
        && path
            .segments
            .iter()
            .all(|segment| matches!(segment.arguments, syn::PathArguments::None))
}
fn safe_value_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => value.attrs.is_empty() && safe_path(&value.path),
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_value_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_value_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_value_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                "BTreeSet::new",
                "BTreeSet::from",
                "ManaCost::new",
                "ManaCost::with_colors",
                "Some",
                "TokenSpec::saproling",
                "ManaAbilityOutput::Fixed",
            ]
            .iter()
            .any(|expected| {
                path.path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
                    == *expected
            })
        }
        Expr::Struct(value) => {
            let path = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            [
                "CardDefinition",
                "ActivatedManaAbility",
                "ActivatedAbility",
                "Effect::CreateToken",
            ]
            .contains(&path.as_str())
                && value.attrs.is_empty()
                && value.rest.is_none()
                && value
                    .fields
                    .iter()
                    .all(|field| safe_value_expr(&field.expr))
        }
        Expr::Macro(value) => {
            if !value.attrs.is_empty() || !value.mac.path.is_ident("vec") {
                return false;
            }
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            parser
                .parse2(value.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(safe_value_expr))
        }
        _ => false,
    }
}

fn validate_candidate_implementation() -> Result<(), String> {
    let submitted = candidate_vitu_ghazi::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vitu-Ghazi is missing from the RAV registry")?;
    if submitted != registered
        || submitted.id != CARD_ID
        || submitted.name != CARD_NAME
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::new(0)
        || !submitted.colors.is_empty()
        || submitted.mana_colors != BTreeSet::from([Color::Colorless])
        || submitted.card_types != BTreeSet::from([CardType::Land])
        || submitted.is_basic_land
        || submitted.power.is_some()
        || submitted.toughness.is_some()
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
    {
        return Err("Vitu-Ghazi printed identity or characteristics differ".to_owned());
    }
    let mana = candidate_vitu_ghazi::colorless_mana_ability();
    let registered_mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Vitu-Ghazi colorless mana binding is missing")?;
    if mana != registered_mana.ability
        || mana.id != "produce-colorless"
        || !mana.tap_cost
        || mana.output != cardbench_magic_engine::ManaAbilityOutput::Fixed(Color::Colorless)
        || mana.amount != 1
        || mana.life_payment.is_some()
        || mana.controller_damage.is_some()
    {
        return Err("Vitu-Ghazi colorless mana ability differs from its RAV identity".to_owned());
    }
    let ability = candidate_vitu_ghazi::create_saproling_ability();
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Vitu-Ghazi Saproling binding is missing")?;
    if ability != registered_ability.ability
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(2, [Color::Green, Color::White])
        || !ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects
            != [Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }]
    {
        return Err(
            "Vitu-Ghazi Saproling ability cost or effect differs from RAV identity".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaColor,
    NoActivationTap,
    NoToken,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    city_trees: Vec<ObjectId>,
    mana_source: Option<ObjectId>,
    token_source: Option<ObjectId>,
    mana_activation_seen: bool,
    colorless_amount: Option<u8>,
    golem: Option<ObjectId>,
    golem_payment: Vec<Color>,
    token: Option<ObjectId>,
    token_live: bool,
    token_stats: Option<(i32, i32)>,
    ability_source_tapped: Option<bool>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_VITU_GHAZI_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_VITU_GHAZI_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no complete Vitu-Ghazi double-land activation trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vitu-Ghazi definition missing")?;
    *definition = candidate_vitu_ghazi::definition();
    let mut mana_bindings = rav_mana_ability_bindings();
    let mana_binding = mana_bindings
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Vitu-Ghazi mana binding missing")?;
    mana_binding.ability = candidate_vitu_ghazi::colorless_mana_ability();
    if mutation == Mutation::WrongManaColor {
        mana_binding.ability.output =
            cardbench_magic_engine::ManaAbilityOutput::Fixed(Color::Green);
    }
    let mut activated = rav_activated_ability_bindings();
    let ability_binding = activated
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Vitu-Ghazi activated binding missing")?;
    ability_binding.ability = candidate_vitu_ghazi::create_saproling_ability();
    match mutation {
        Mutation::NoActivationTap => ability_binding.ability.tap_cost = false,
        Mutation::NoToken => ability_binding.ability.effects.clear(),
        _ => {}
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        mana_bindings,
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
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-GLASS-GOLEM".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-WATCHWOLF".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
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
    if [&deck0, &deck1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Vitu-Ghazi fixtures must use legal shuffled 60-card decks".to_owned());
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
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(CityTreePolicy::new(PlayerId(0))),
        Box::new(PassPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        city_trees: vec![],
        mana_source: None,
        token_source: None,
        mana_activation_seen: false,
        colorless_amount: None,
        golem: None,
        golem_payment: vec![],
        token: None,
        token_live: false,
        token_stats: None,
        ability_source_tapped: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted >= config.max_policy_moves {
            return Err(format!(
                "Vitu-Ghazi game exceeded its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Vitu-Ghazi policy seat missing")?;
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
        let source = match &action {
            PolicyAction::ActivateManaAbility { land, .. }
                if view
                    .own_battlefield
                    .iter()
                    .any(|card| card.id == *land && card.definition == Some(CARD_ID)) =>
            {
                Some((*land, true))
            }
            PolicyAction::ActivateBoundManaAbility { activation }
                if view.own_battlefield.iter().any(|card| {
                    card.id == activation.source && card.definition == Some(CARD_ID)
                }) =>
            {
                Some((activation.source, true))
            }
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some((activation.source, false))
            }
            _ => None,
        };
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Vitu-Ghazi move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if let Some((source, is_mana)) = source {
            if is_mana {
                captured.mana_source.get_or_insert(source);
            } else {
                captured.token_source.get_or_insert(source);
            }
            captured.ability_source_tapped =
                Some(game.object(source).is_ok_and(|object| object.tapped));
        }
        observe(&game, &game.event_log[before..], &mut captured)?;
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

fn observe(game: &Game, events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                if !captured.city_trees.contains(card) {
                    captured.city_trees.push(*card);
                }
            }
            GameEvent::BoundManaAbilityActivated {
                player: PlayerId(0),
                source,
                ability: "produce-colorless",
                color: Color::Colorless,
                amount: 1,
                tapped: true,
                ..
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.mana_activation_seen = true;
                captured.mana_source.get_or_insert(*source);
                captured.colorless_amount = Some(1);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == "RAV-GLASS-GOLEM") =>
            {
                captured.golem = Some(*card);
                captured.golem_payment = colors.clone();
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                definition: CARD_ID,
                ability: ABILITY_ID,
                ..
            } => {
                captured.token_source = Some(*source);
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => captured.token = Some(*token),
            _ => {}
        }
    }
    if let Some(token) = captured.token {
        if game.zone_of(token) == Some(Zone::Battlefield) {
            let characteristics = game
                .characteristics(token)
                .map_err(|error| error.to_string())?;
            captured.token_live = game
                .object(token)
                .is_ok_and(|object| object.token == Some(TokenSpec::saproling()))
                && game
                    .controller_of(token)
                    .is_ok_and(|controller| controller == PlayerId(0))
                && characteristics.colors == BTreeSet::from([Color::Green])
                && characteristics.card_types.contains(&CardType::Creature)
                && characteristics
                    .creature_subtypes
                    .contains(&cardbench_magic_engine::CreatureSubtype::Saproling);
            captured.token_stats = Some((
                characteristics.power.ok_or("Saproling has no power")?,
                characteristics
                    .toughness
                    .ok_or("Saproling has no toughness")?,
            ));
        }
    }
    Ok(())
}

struct CityTreePolicy {
    player: PlayerId,
    activated_token: bool,
    cast_golem: bool,
    cast_watchwolf: bool,
}
impl CityTreePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activated_token: false,
            cast_golem: false,
            cast_watchwolf: false,
        }
    }
    fn count_trees(view: &GameView) -> usize {
        view.own_battlefield
            .iter()
            .filter(|card| card.definition == Some(CARD_ID))
            .count()
    }
    fn mana(view: &GameView, color: Color, excluded: Option<ObjectId>) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                Some(card.id) != excluded
                    && !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color,
            })
    }
    fn land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let card = if Self::count_trees(view) == 0 {
            view.hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
        } else {
            view.hand
                .iter()
                .find(|card| {
                    card.card_types.contains(&CardType::Land) && card.definition != Some(CARD_ID)
                })
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
        }?;
        Some(PolicyAction::PlayLand { card: card.id })
    }
    fn cast_golem(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.cast_golem {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-GLASS-GOLEM"))?;
        if Self::count_trees(view) == 0
            || view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count()
                < 5
        {
            return None;
        }
        if view.mana_pool.amount(Color::Colorless) == 0 {
            if let Some(tree) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && !card.tapped)
            {
                return Some(PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: tree.id,
                        ability_id: "produce-colorless",
                        chosen_color: None,
                    },
                });
            }
        }
        if view.mana_pool.total() < 5 {
            return view
                .own_battlefield
                .iter()
                .find(|land| {
                    land.card_types.contains(&CardType::Land)
                        && land.definition != Some(CARD_ID)
                        && !land.tapped
                })
                .map(|land| PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: if land.definition == Some("RAV-FOREST") {
                        Color::Green
                    } else {
                        Color::White
                    },
                });
        }
        let mana_selection = generic_payment(view, 5)?;
        self.cast_golem = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection,
        })
    }
    fn token_ability(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.activated_token || Self::count_trees(view) == 0 || !self.cast_golem {
            return None;
        }
        let source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && !card.tapped)?
            .id;
        let pool = &view.mana_pool;
        if pool.amount(Color::Green) == 0 {
            return Self::mana(view, Color::Green, Some(source));
        }
        if pool.amount(Color::White) == 0 {
            return Self::mana(view, Color::White, Some(source));
        }
        if pool.total() < 4 {
            return Color::MANA_ALL
                .into_iter()
                .find_map(|color| Self::mana(view, color, Some(source)));
        }
        self.activated_token = true;
        Some(PolicyAction::ActivateAbility {
            activation: cardbench_magic_engine::AbilityActivation {
                source,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }
    fn cast_watchwolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.cast_watchwolf {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-WATCHWOLF"))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return Self::mana(view, Color::Green, None);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return Self::mana(view, Color::White, None);
        }
        self.cast_watchwolf = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: Default::default(),
        })
    }
}
impl CodePolicy for CityTreePolicy {
    fn id(&self) -> &'static str {
        "rav.vitu-ghazi-city-tree-pilot.v1"
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = Self::land(view) {
            return action;
        }
        if let Some(action) = self.cast_golem(view) {
            return action;
        }
        if let Some(action) = self.token_ability(view) {
            return action;
        }
        if let Some(action) = self.cast_watchwolf(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}
fn generic_payment(view: &GameView, count: usize) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    let mut generic = Vec::with_capacity(count);
    for _ in 0..count {
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
struct PassPolicy {
    player: PlayerId,
}
impl PassPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.vitu-ghazi-pass-opponent.v1"
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
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Vitu-Ghazi game did not reach a clean seat-zero win".to_owned());
    }
    let trees: Vec<_> = captured.city_trees.iter().copied().collect();
    if trees.is_empty() {
        return Err("Vitu-Ghazi did not enter under the pilot's control".to_owned());
    }
    let mana_source = captured
        .mana_source
        .ok_or("Vitu-Ghazi colorless mana ability was not activated")?;
    let token_source = captured
        .token_source
        .ok_or("Vitu-Ghazi Saproling ability was not activated")?;
    if mana_source != token_source
        || !trees.contains(&mana_source)
        || !trees.contains(&token_source)
    {
        return Err("the same Vitu-Ghazi copy must supply colorless mana and later activate its Saproling ability".to_owned());
    }
    let golem = captured
        .golem
        .ok_or("Glass Golem generic-cost fixture was not cast")?;
    if captured.golem_payment.len() != 5
        || captured
            .golem_payment
            .iter()
            .filter(|color| **color == Color::Colorless)
            .count()
            != 1
    {
        return Err(format!(
            "Vitu-Ghazi colorless output was not used to pay exactly one generic mana of Glass Golem: {:?}",
            captured.golem_payment
        ));
    }
    if !captured.mana_activation_seen || captured.colorless_amount != Some(1) {
        let mana_events: Vec<_> = captured
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::BoundManaAbilityActivated { .. }
                        | GameEvent::ManaAbilityActivated { .. }
                        | GameEvent::ManaAdded { .. }
                        | GameEvent::SpellManaPaid { .. }
                )
            })
            .collect();
        return Err(format!(
            "Vitu-Ghazi did not produce exactly one colorless mana: {mana_events:?}"
        ));
    }
    if captured.ability_source_tapped != Some(true) {
        return Err("Vitu-Ghazi Saproling activation did not pay its tap cost".to_owned());
    }
    if !captured.token_live || captured.token_stats != Some((1, 1)) {
        return Err(
            "Vitu-Ghazi ability did not leave a controlled green 1/1 Saproling on the battlefield"
                .to_owned(),
        );
    }
    let events = &captured.events;
    let mana_activation = unique_index(events, |event| matches!(event, GameEvent::BoundManaAbilityActivated { player: PlayerId(0), source, ability: "produce-colorless", color: Color::Colorless, amount: 1, tapped: true, .. } if *source == mana_source)).ok_or("typed colorless mana-ability activation receipt missing")?;
    let colorless_added = first_after(events, mana_activation, |event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player: PlayerId(0),
                color: Color::Colorless,
                amount: 1
            }
        )
    })
    .ok_or("colorless mana output receipt missing")?;
    let golem_payment = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == golem && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Colorless).count() == 1)).ok_or("Glass Golem did not spend exactly one City-Tree colorless mana in its five-generic cost")?;
    let golem_cast = first_after(events, golem_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == golem)).ok_or("Glass Golem cast receipt missing")?;
    let golem_resolved = first_after(
        events,
        golem_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == golem),
    )
    .ok_or("Glass Golem did not resolve")?;
    let activation = first_after(events, golem_resolved, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == token_source)).ok_or("Saproling ability did not reach the stack after the colorless-paid fixture spell")?;
    let paid = first_after(events, golem_resolved, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == token_source && *mana_cost == ManaCost::with_colors(2, [Color::Green, Color::White]))).ok_or("Saproling activation did not pay the exact {2}{G}{W} cost")?;
    let resolved = first_after(events, activation, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == token_source)).ok_or("Saproling ability failed to resolve")?;
    let token_event = unique_index(events, |event| matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if Some(*token) == captured.token)).ok_or("token creation receipt missing")?;
    let attack = first_after(events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&captured.token.unwrap()))).ok_or("Saproling did not become eligible to attack on a later turn")?;
    let damage = first_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if Some(*source) == captured.token)).ok_or("Saproling did not deal its live one combat damage")?;
    let ended = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded event missing")?;
    if mana_activation >= colorless_added
        || colorless_added >= golem_payment
        || golem_payment >= golem_cast
        || golem_cast >= golem_resolved
        || golem_resolved >= paid
        || paid >= activation
        || activation >= token_event
        || token_event >= resolved
        || resolved >= attack
        || attack >= damage
        || damage >= ended
    {
        return Err(
            "Vitu-Ghazi mana, activation, token, combat, and terminal event order differs"
                .to_owned(),
        );
    }
    let witness: BTreeSet<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        let include = matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if trees.contains(card))
            || matches!(event, GameEvent::BoundManaAbilityActivated { player: PlayerId(0), source, ability: "produce-colorless", .. } if *source == mana_source)
            || matches!(event, GameEvent::ManaAdded { player: PlayerId(0), color: Color::Colorless, amount: 1 })
            || matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == golem && colors.len() == 5)
            || matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == golem)
            || matches!(event, GameEvent::SpellResolved { card } if *card == golem)
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == golem)
            || matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == token_source)
            || matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == token_source)
            || matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == token_source)
            || matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if Some(*token) == captured.token)
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if captured.token.is_some_and(|token| attackers.contains(&token)))
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if Some(*source) == captured.token)
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        include.then_some(index)
    }).collect();
    let projected_all = project_events(events);
    let ordered_event_sequence: Vec<_> = witness
        .iter()
        .map(|index| projected_all[*index].sequence)
        .collect();
    let projected: Vec<Value> = witness
        .iter()
        .map(|index| {
            serde_json::to_value(&projected_all[*index]).expect("projected event serializes")
        })
        .collect();
    Ok(json!({
        "id": "rav-vitu-ghazi-colorless-to-saproling-full-game",
        "seed": seed,
        "passed": true,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "city_tree_mana_source": mana_source.0,
        "city_tree_token_source": token_source.0,
        "same_city_tree_reused_after_untap": mana_source == token_source,
        "digest": event_digest(events),
        "ordered_event_sequence": ordered_event_sequence,
        "colorless_mana_added": captured.colorless_amount,
        "generic_fixture_spell": {"definition_id": "RAV-GLASS-GOLEM", "object": golem.0, "payment": captured.golem_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "saproling": {"object": captured.token.map(|token| token.0), "controller_seat": 0, "zone": "Battlefield", "green_creature_saproling": captured.token_live, "power": 1, "toughness": 1},
        "activation_source_tapped": captured.ability_source_tapped,
        "mana_ability_before_token_activation": mana_activation < activation,
        "event_count": events.len(),
        "events": projected
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut found = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}
fn first_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for event in events {
        for byte in format!("{event:?}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .map(|captured| validate_trace(&captured, seed).is_err())
        .unwrap_or(true)
}
fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.city_trees == right.city_trees
        && left.mana_source == right.mana_source
        && left.token_source == right.token_source
        && left.mana_activation_seen == right.mana_activation_seen
        && left.colorless_amount == right.colorless_amount
        && left.golem == right.golem
        && left.golem_payment == right.golem_payment
        && left.token == right.token
        && left.token_live == right.token_live
        && left.token_stats == right.token_stats
        && left.ability_source_tapped == right.ability_source_tapped
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 285;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_only" } else { "not_evaluated" }, "credited": false})
}
