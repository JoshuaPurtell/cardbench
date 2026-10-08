//! Full-game diagnostic authority for RAV Guardian of Vitu-Ghazi.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ConvokeContribution, ConvokePayment, DeckEntry, DeckList,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Zone,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GUARDIAN-OF-VITU-GHAZI";
const CARD_NAME: &str = "Guardian of Vitu-Ghazi";
const ELVES_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SCATTER_ID: &str = "RAV-SCATTER-THE-SEEDS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/guardian_of_vitu_ghazi.rs";
const MAX_TRACE_SEED: u64 = 96;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/guardian_of_vitu_ghazi.rs"]
mod candidate_guardian;

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
            eprintln!("Guardian of Vitu-Ghazi PvP authority failed: {error}");
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
    if !same_match(&captured, &replay)
        || project_events(&captured.events) != project_events(&replay.events)
    {
        return Err("Guardian shuffled full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "convoke_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "generic_cost_increased_mutation_detected_in_game": mutation_detected(seed, Mutation::MoreGeneric),
        "vigilance_removed_mutation_detected_in_combat": mutation_detected(seed, Mutation::NoVigilance),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["convoke_removed_mutation_detected_in_game"] != true
        || mutation_sensitivity["generic_cost_increased_mutation_detected_in_game"] != true
        || mutation_sensitivity["vigilance_removed_mutation_detected_in_combat"] != true
    {
        return Err("a Guardian Convoke/Vigilance semantic mutation escaped".to_owned());
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
        "card": {"collector_number": 212, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2 Casting a spell and paying costs",
                "702.50 Convoke",
                "702.20 Vigilance",
                "508 Declaring attackers",
                "510 Combat damage and state-based actions"
            ],
            "source_contract": "RAV Guardian of Vitu-Ghazi is a green-white 4/7 Creature with {6}{G}{W}, Convoke, and Vigilance",
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
        "qualification_status": "diagnostic_only; external qualification pending",
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

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_guardian::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Guardian of Vitu-Ghazi is missing from the RAV registry")?;
    if definition != runtime
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(6, [Color::Green, Color::White])
        || definition.colors != BTreeSet::from([Color::Green, Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(4)
        || definition.toughness != Some(7)
        || definition.keywords != [Keyword::Convoke, Keyword::Vigilance]
        || !definition.effects.is_empty()
    {
        return Err(
            "Guardian identity, {6}{G}{W} cost, 4/7 body, Convoke, or Vigilance differs".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Guardian source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Guardian source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Guardian module contains unsupported attributes".to_owned());
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
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut imports)?;
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => return Err("Guardian source contains executable module items".to_owned()),
        }
    }
    if imports != expected_imports || file.items.len() != 3 {
        return Err("Guardian module imports or shape differ from the source contract".to_owned());
    }
    let function = definition.ok_or("Guardian definition() is missing")?;
    if function.sig.ident != "definition"
        || !function
            .attrs
            .iter()
            .all(|attr| attr.path().is_ident("doc") || attr.path().is_ident("must_use"))
        || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if path_is_type(ty, "CardDefinition"))
        || function.block.stmts.len() != 1
    {
        return Err("Guardian definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
        return Err("Guardian definition() must return one direct struct literal".to_owned());
    };
    let fields = literal_fields(literal)?;
    if !path_is(&literal.path, &["CardDefinition"])
        || literal.rest.is_some()
        || fields.len() != 13
        || !fields.values().all(|expr| safe_expr(expr))
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], CARD_NAME)
        || !check_string(fields["set_code"], "RAV")
        || !check_cost(fields["mana_cost"])
        || !check_set(
            fields["colors"],
            &[&["Color", "Green"], &["Color", "White"]],
        )
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Creature"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(fields["supported_rules"])
        || !check_some_int(fields["power"], 4)
        || !check_some_int(fields["toughness"], 7)
        || !check_keywords(fields["keywords"])
        || !check_empty_vec(fields["effects"])
    {
        return Err("Guardian literal is outside the task source contract".to_owned());
    }
    Ok(())
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
        UseTree::Rename(_) | UseTree::Glob(_) => {
            Err("Guardian import aliases and globs are forbidden".to_owned())
        }
    }
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(segment.arguments, syn::PathArguments::None)
        })
}
fn path_is_type(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[expected]))
}
fn literal_fields<'a>(literal: &'a syn::ExprStruct) -> Result<BTreeMap<String, &'a Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &literal.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Guardian literal requires named fields".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Guardian literal has duplicate or shorthand fields".to_owned());
        }
    }
    Ok(fields)
}
fn safe_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            value.attrs.is_empty()
                && [
                    "Color::Green",
                    "Color::White",
                    "CardType::Creature",
                    "Keyword::Convoke",
                    "Keyword::Vigilance",
                ]
                .iter()
                .any(|path| {
                    value
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("::")
                        == *path
                })
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                "BTreeSet::from",
                "BTreeSet::new",
                "ManaCost::with_colors",
                "Some",
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
        Expr::Macro(value) => {
            value.attrs.is_empty()
                && value.mac.path.is_ident("vec")
                && (value.mac.tokens.is_empty()
                    || value.mac.tokens.to_string() == "Keyword :: Convoke , Keyword :: Vigilance")
        }
        _ => false,
    }
}
fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Str(value) if value.value() == expected))
}
fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Bool(value) if value.value == expected))
}
fn check_int(expr: &Expr, expected: i64) -> bool {
    matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(value) if value.base10_parse::<i64>().ok() == Some(expected)))
}
fn call_args<'a>(expr: &'a Expr, owner: &str, method: &str) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else { return None };
    let Expr::Path(path) = &*call.func else {
        return None;
    };
    (path_is(&path.path, &[owner, method]) && call.attrs.is_empty())
        .then(|| call.args.iter().collect())
}
fn check_empty_set(expr: &Expr) -> bool {
    call_args(expr, "BTreeSet", "new").is_some_and(|args| args.is_empty())
}
fn check_set(expr: &Expr, expected: &[&[&str]]) -> bool {
    let Some(args) = call_args(expr, "BTreeSet", "from") else {
        return false;
    };
    if args.len() != 1 {
        return false;
    }
    matches!(args[0], Expr::Array(array) if array.elems.len() == expected.len() && array.elems.iter().zip(expected).all(|(item, path)| matches!(item, Expr::Path(value) if path_is(&value.path, path))))
}
fn check_cost(expr: &Expr) -> bool {
    let Some(args) = call_args(expr, "ManaCost", "with_colors") else {
        return false;
    };
    if args.len() != 2 || !check_int(args[0], 6) {
        return false;
    }
    matches!(args[1], Expr::Array(array) if array.elems.len() == 2 && matches!(&array.elems[0], Expr::Path(path) if path_is(&path.path, &["Color", "Green"])) && matches!(&array.elems[1], Expr::Path(path) if path_is(&path.path, &["Color", "White"])))
}
fn check_rules(expr: &Expr) -> bool {
    let Expr::Reference(reference) = expr else {
        return false;
    };
    matches!(&*reference.expr, Expr::Array(array) if array.elems.len() == 4 && ["full-rules-fidelity", "convoke", "base-characteristics", "vigilance"].iter().zip(array.elems.iter()).all(|(expected, item)| check_string(item, expected)))
}
fn check_some_int(expr: &Expr, expected: i64) -> bool {
    let Some(args) = call_args(expr, "Some", "") else {
        if let Expr::Call(call) = expr {
            return matches!(&*call.func, Expr::Path(path) if path_is(&path.path, &["Some"]))
                && call.args.len() == 1
                && check_int(&call.args[0], expected);
        }
        return false;
    };
    args.len() == 1 && check_int(args[0], expected)
}
fn check_keywords(expr: &Expr) -> bool {
    matches!(expr, Expr::Macro(value) if value.mac.path.is_ident("vec") && value.mac.tokens.to_string() == "Keyword :: Convoke , Keyword :: Vigilance")
}
fn check_empty_vec(expr: &Expr) -> bool {
    matches!(expr, Expr::Macro(value) if value.mac.path.is_ident("vec") && value.mac.tokens.is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    MoreGeneric,
    NoVigilance,
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    guardian: Option<ObjectId>,
    scatter: Option<ObjectId>,
    convokers: Vec<ObjectId>,
    convoker_tapped: Vec<bool>,
    payment_colors: Vec<Color>,
    fixture_tokens: Vec<ObjectId>,
    live_stats_at_entry: Option<(i32, i32, bool, bool)>,
    live_stats_at_attack: Option<(i32, i32, bool, bool)>,
    attack_untapped: Option<bool>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GUARDIAN_VITU_GHAZI_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_GUARDIAN_VITU_GHAZI_SEED must be an unsigned integer".to_owned()
        })?;
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
        "no terminal Guardian Convoke/Vigilance full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_guardian::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::MoreGeneric => {
            definition.mana_cost = ManaCost::with_colors(7, [Color::Green, Color::White])
        }
        Mutation::NoVigilance => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Vigilance),
    }
    let mut game = new_rav_game(definition)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ELVES_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SCATTER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ELVES_ID.to_owned(),
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
    if [&deck0, &deck1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Guardian fixtures must use legal shuffled 60-card decks".to_owned());
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
        Box::new(GuardianPolicy::new(PlayerId(0))),
        Box::new(OpponentPolicy::new(PlayerId(1))),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        guardian: None,
        scatter: None,
        convokers: vec![],
        convoker_tapped: vec![],
        payment_colors: vec![],
        fixture_tokens: vec![],
        live_stats_at_entry: None,
        live_stats_at_attack: None,
        attack_untapped: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Guardian full game hit its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Guardian policy seat missing")?;
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
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Guardian policy action failed on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    captured.rejected_moves = 0;
    Ok(captured)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut CapturedMatch) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.guardian = Some(*card)
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == SCATTER_ID) =>
            {
                captured.scatter = Some(*card)
            }
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } => {
                captured.convokers.push(*creature);
                captured.convoker_tapped.push(
                    game.object(*creature)
                        .map_err(|error| error.to_string())?
                        .tapped,
                );
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.payment_colors = colors.clone()
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => captured.fixture_tokens.push(*token),
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured
                .guardian
                .is_some_and(|guardian| attackers.contains(&guardian)) =>
            {
                let guardian = captured.guardian.expect("guarded by predicate");
                let object = game.object(guardian).map_err(|error| error.to_string())?;
                captured.attack_untapped.get_or_insert(!object.tapped);
                let stats = game
                    .characteristics(guardian)
                    .map_err(|error| error.to_string())?;
                captured.live_stats_at_attack.get_or_insert((
                    stats.power.ok_or("Guardian has no live power")?,
                    stats.toughness.ok_or("Guardian has no live toughness")?,
                    stats.keywords.contains(&Keyword::Convoke),
                    stats.keywords.contains(&Keyword::Vigilance),
                ));
            }
            _ => {}
        }
    }
    if let Some(guardian) = captured.guardian
        && captured.live_stats_at_entry.is_none()
        && game.zone_of(guardian) == Some(Zone::Battlefield)
    {
        let stats = game
            .characteristics(guardian)
            .map_err(|error| error.to_string())?;
        captured.live_stats_at_entry = Some((
            stats.power.ok_or("Guardian has no live power")?,
            stats.toughness.ok_or("Guardian has no live toughness")?,
            stats.keywords.contains(&Keyword::Convoke),
            stats.keywords.contains(&Keyword::Vigilance),
        ));
    }
    Ok(())
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Guardian registry definition missing")?;
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

struct GuardianPolicy {
    player: PlayerId,
}
impl GuardianPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
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
    fn land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let forests = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-FOREST"))
            .count();
        let plains = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-PLAINS"))
            .count();
        let desired = if plains == 0 && forests >= 5 {
            "RAV-PLAINS"
        } else {
            "RAV-FOREST"
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(desired))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
    fn creature_count(view: &GameView) -> usize {
        view.own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Creature))
            .count()
    }
    fn cast_elf(view: &GameView) -> Option<PolicyAction> {
        if Self::creature_count(view) >= 3 {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(ELVES_ID))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return Self::mana_source(view, Color::Green);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
    fn cast_scatter(view: &GameView) -> Option<PolicyAction> {
        if Self::creature_count(view) != 3
            || view
                .hand
                .iter()
                .all(|card| card.definition != Some(SCATTER_ID))
        {
            return None;
        }
        let lands = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land))
            .count();
        if lands < 5 {
            return None;
        }
        let spell = view
            .hand
            .iter()
            .find(|card| card.definition == Some(SCATTER_ID))?;
        let cost = ManaCost::with_colors(3, [Color::Green, Color::Green]);
        let Some(selection) = payment_selection(view, &cost) else {
            for color in [Color::Green, Color::Green] {
                if view.mana_pool.amount(color) == 0 {
                    return Self::mana_source(view, color);
                }
            }
            return Color::MANA_ALL
                .into_iter()
                .find_map(|color| Self::mana_source(view, color));
        };
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: spell.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: selection,
        })
    }
    fn cast_guardian(view: &GameView) -> Option<PolicyAction> {
        if view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
            || view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(CARD_ID))
        {
            return None;
        }
        let contributors: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Creature) && !card.tapped)
            .take(6)
            .collect();
        if contributors.len() != 6 {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        for color in [Color::Green, Color::White] {
            if view.mana_pool.amount(color) == 0 {
                return Self::mana_source(view, color);
            }
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: contributors
                    .iter()
                    .map(|creature| ConvokePayment {
                        creature: creature.id,
                        contribution: ConvokeContribution::Generic,
                    })
                    .collect(),
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
}
impl CodePolicy for GuardianPolicy {
    fn id(&self) -> &'static str {
        "rav.guardian-vitu-ghazi-convoke-vigilance.v1"
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
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = Self::land(view) {
            return action;
        }
        if let Some(action) = Self::cast_elf(view) {
            return action;
        }
        if let Some(action) = Self::cast_scatter(view) {
            return action;
        }
        if let Some(action) = Self::cast_guardian(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct OpponentPolicy {
    player: PlayerId,
    cast_one_elf: bool,
}
impl OpponentPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast_one_elf: false,
        }
    }
}
impl CodePolicy for OpponentPolicy {
    fn id(&self) -> &'static str {
        "rav.guardian-opponent-one-elf.v1"
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
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.cast_one_elf
            && view
                .own_battlefield
                .iter()
                .all(|card| card.definition != Some(ELVES_ID))
        {
            if let Some(elf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ELVES_ID))
            {
                if view.mana_pool.amount(Color::Green) == 0 {
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
                }
                self.cast_one_elf = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: elf.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let slot = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
        if slot.1 == 0 {
            return None;
        }
        slot.1 -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let index = available
            .iter()
            .position(|(color, count)| *count > 0 && *color == Color::Green)
            .or_else(|| available.iter().position(|(_, count)| *count > 0))?;
        let slot = &mut available[index];
        slot.1 -= 1;
        generic.push(slot.0);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Guardian full-game trace did not end in a P0 win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Guardian trace contains rejected policy moves".to_owned());
    }
    let guardian = captured.guardian.ok_or_else(|| format!(
        "Guardian was not cast (terminal={}, turns={}, moves={}, spells={:?}, tokens={}, convoke={})",
        captured.terminal, captured.turns, captured.accepted_moves,
        captured.events.iter().filter(|event| matches!(event, GameEvent::SpellCast { .. })).collect::<Vec<_>>(),
        captured.fixture_tokens.len(), captured.convokers.len()
    ))?;
    let scatter = captured
        .scatter
        .ok_or("Scatter the Seeds fixture was not cast")?;
    if captured.convokers.len() != 6
        || captured.convokers.iter().collect::<BTreeSet<_>>().len() != 6
        || captured.convoker_tapped != [true; 6]
    {
        return Err(format!(
            "Guardian did not tap six distinct controlled creatures for Generic Convoke: {:?} / {:?}",
            captured.convokers, captured.convoker_tapped
        ));
    }
    let mut colors = captured.payment_colors.clone();
    colors.sort_by_key(|color| format!("{color:?}"));
    if colors != [Color::Green, Color::White] {
        return Err(format!(
            "Guardian must pay exactly Green and White after six generic Convoke contributions, got {:?}",
            captured.payment_colors
        ));
    }
    if captured.fixture_tokens.len() != 3 {
        return Err(
            "Scatter the Seeds did not create three creatures for the six-convoker fixture"
                .to_owned(),
        );
    }
    if captured.live_stats_at_entry != Some((4, 7, true, true)) {
        return Err("live Guardian entry is not 4/7 with Convoke and Vigilance".to_owned());
    }
    if captured.live_stats_at_attack != Some((4, 7, true, true)) {
        return Err(
            "Guardian did not retain its live characteristics/keywords when attacking".to_owned(),
        );
    }
    if captured.attack_untapped != Some(true) {
        return Err("Vigilance attack left Guardian tapped".to_owned());
    }
    let events = &captured.events;
    let convoke: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature)).then_some(index)).collect();
    if convoke.len() != 6 {
        return Err("typed trace lacks six Generic Convoke receipts".to_owned());
    }
    let payment = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == guardian && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("exact Green and White mana-payment receipt is missing")?;
    let cast = unique_event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == guardian)).ok_or("Guardian SpellCast is missing")?;
    let resolution = unique_event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == guardian),
    )
    .ok_or("Guardian SpellResolved is missing")?;
    let entry = unique_event_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == guardian)).ok_or("Guardian battlefield entry is missing")?;
    let scatter_cast = unique_event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == scatter)).ok_or("Scatter SpellCast is missing")?;
    let scatter_resolved = unique_event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == scatter),
    )
    .ok_or("Scatter SpellResolved is missing")?;
    let attack = first_event_index(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&guardian))).ok_or("Guardian did not attack")?;
    let blockers = first_event_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("opponent did not make the empty legal blocker declaration")?;
    let damage = first_event_index(events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == guardian)).ok_or("Guardian did not deal four combat damage")?;
    let end = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("P0 GameEnded event is missing")?;
    if convoke.iter().any(|index| *index >= payment)
        || payment >= cast
        || cast >= resolution
        || resolution >= entry
        || scatter_cast >= scatter_resolved
        || scatter_resolved >= convoke[0]
        || entry >= attack
        || attack >= blockers
        || blockers >= damage
        || damage >= end
    {
        return Err(
            "Guardian payment/cast/entry/vigilance combat/end event order differs".to_owned(),
        );
    }
    let witness: BTreeSet<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        let is_fixture = matches!(event, GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } | GameEvent::CardMoved { card, .. } if *card == scatter);
        let include = is_fixture || matches!(event, GameEvent::TokenCreated { player: PlayerId(0), .. })
            || matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature))
            || matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } | GameEvent::SpellCast { player: PlayerId(0), card } | GameEvent::SpellResolved { card } if *card == guardian)
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == guardian)
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&guardian))
            || index == blockers
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == guardian)
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        include.then_some(index)
    }).collect();
    let projected_all = project_events(events);
    let mut projected = Vec::new();
    for index in &witness {
        let mut value =
            serde_json::to_value(projected_all.get(*index).ok_or("projected event missing")?)
                .map_err(|error| error.to_string())?;
        if let GameEvent::ConvokeUsed {
            player,
            creature,
            contribution: None,
        } = &events[*index]
        {
            value["facts"] =
                json!({"seats": [player.0], "objects": [creature.0], "contribution": "Generic"});
        }
        if let GameEvent::AttackersDeclared { player, attackers } = &events[*index] {
            if attackers.contains(&guardian) {
                value["facts"] = json!({"seats": [player.0], "objects": [guardian.0], "guardian_untapped": captured.attack_untapped});
            }
        }
        projected.push(value);
    }
    Ok(json!({
        "trace_id": "rav-guardian-convoke-vigilance-full-game",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "rejected_policy_moves": captured.rejected_moves,
        "full_game_event_count": events.len(),
        "mechanics": {
            "guardian": guardian.0,
            "fixture_spell": scatter.0,
            "mana_cost": "{6}{G}{W}",
            "generic_paid_by_convoke": 6,
            "convoke_contribution": "Generic",
            "convoke_contributors": captured.convokers.iter().map(|object| object.0).collect::<Vec<_>>(),
            "convoke_creatures_tapped": captured.convoker_tapped,
            "colored_mana_paid": colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "live_characteristics_at_entry": {"power": 4, "toughness": 7, "convoke": true, "vigilance": true},
            "live_characteristics_at_attack": {"power": 4, "toughness": 7, "convoke": true, "vigilance": true},
            "vigilance_attacker_untapped": captured.attack_untapped,
            "fixture_tokens_created": captured.fixture_tokens.iter().map(|object| object.0).collect::<Vec<_>>(),
            "attacked_and_dealt_four_combat_damage": true
        },
        "events": projected
    }))
}

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matching = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matching.next()?;
    matching.next().is_none().then_some(first)
}
fn first_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(predicate)
}
fn first_event_after(
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
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}
fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.guardian == right.guardian
        && left.scatter == right.scatter
        && left.convokers == right.convokers
        && left.convoker_tapped == right.convoker_tapped
        && left.payment_colors == right.payment_colors
        && left.fixture_tokens == right.fixture_tokens
        && left.live_stats_at_entry == right.live_stats_at_entry
        && left.live_stats_at_attack == right.live_stats_at_attack
        && left.attack_untapped == right.attack_untapped
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 212;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_only" } else { "not_evaluated" }, "credited": false})
}
