//! Full-game RAV Conclave Equenaut Convoke and Flying authority.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, ConvokeContribution, ConvokePayment, DeckEntry,
    DeckList, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
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
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CONCLAVE-EQUENAUT";
const BIRDS: &str = "RAV-BIRDS-OF-PARADISE";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/conclave_equenaut.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/conclave_equenaut.rs"]
mod candidate_conclave_equenaut;

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
            eprintln!("Conclave Equenaut PvP authority failed: {error}");
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
    if !same_match(&captured, &replay) {
        return Err("Conclave Equenaut shuffled full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "convoke_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "generic_cost_increased_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost),
        "flying_removed_mutation_detected_in_block_legality": mutation_detected(seed, Mutation::NoFlying),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["convoke_removed_mutation_detected_in_game"] != true
        || mutation_sensitivity["generic_cost_increased_mutation_detected_in_game"] != true
        || mutation_sensitivity["flying_removed_mutation_detected_in_block_legality"] != true
    {
        return Err("a Conclave Equenaut semantic mutation escaped the gameplay oracle".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 9, "definition_id": CARD_ID, "name": "Conclave Equenaut"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2 Casting a spell and paying costs",
                "702.50 Convoke",
                "702.9 Flying",
                "508 Declaring attackers",
                "509 Declaring blockers and evasion",
                "510 Combat damage and state-based actions"
            ],
            "source_contract": "RAV Conclave Equenaut identity, {4}{W}{W} cost, white 3/3 Creature, Convoke, and Flying",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
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
    let definition = candidate_conclave_equenaut::definition();
    if definition.id != CARD_ID
        || definition.name != "Conclave Equenaut"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(4, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(3)
        || definition.toughness != Some(3)
        || definition.keywords != [Keyword::Convoke, Keyword::Flying]
        || !definition.effects.is_empty()
    {
        return Err(
            "Conclave Equenaut typed definition differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

/// The submitted module must be a single declarative CardDefinition literal.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Conclave Equenaut source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Conclave Equenaut source is not valid Rust".to_owned())?;
    let expected: BTreeSet<Vec<String>> = [
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
    let mut function = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Conclave Equenaut source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item) if function.is_none() => function = Some(item),
            _ => return Err("Conclave Equenaut source contains executable module items".to_owned()),
        }
    }
    if imports != expected || file.items.len() != 3 || !file.attrs.iter().all(is_doc_attribute) {
        return Err(
            "Conclave Equenaut module structure differs from the source contract".to_owned(),
        );
    }
    let function = function.ok_or("Conclave Equenaut definition() is missing")?;
    if function.sig.ident != "definition"
        || !function.attrs.iter().all(|attr| {
            is_doc_attribute(attr)
                || attr.path().is_ident("must_use") && matches!(attr.meta, Meta::Path(_))
        })
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
        return Err("Conclave Equenaut definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Conclave Equenaut definition() must return one direct literal".to_owned());
    };
    if !path_is(&value.path, &["CardDefinition"]) || value.rest.is_some() || !value.attrs.is_empty()
    {
        return Err("Conclave Equenaut has the wrong literal type".to_owned());
    }
    let fields = literal_fields(value)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Conclave Equenaut")
        || !check_string(fields["set_code"], "RAV")
        || !check_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "White"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Creature"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(fields["supported_rules"])
        || !check_some_integer(fields["power"], 3)
        || !check_some_integer(fields["toughness"], 3)
        || !check_keywords(fields["keywords"])
        || !check_empty_vec(fields["effects"])
    {
        return Err("Conclave Equenaut literal is outside the task source contract".to_owned());
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn is_doc_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("doc")
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(&segment.arguments, syn::PathArguments::None)
        })
}
fn path_is_type(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[expected]))
}
fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("named fields required".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("duplicate or shorthand CardDefinition field".to_owned());
        }
    }
    Ok(fields)
}
fn is_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}
fn call_args<'a>(expr: &'a Expr, owner: &str, method: &str) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else { return None };
    let Expr::Path(path) = &*call.func else {
        return None;
    };
    (path.qself.is_none() && path_is(&path.path, &[owner, method]) && call.attrs.is_empty())
        .then(|| call.args.iter().collect())
}
fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(lit) if lit.attrs.is_empty() && matches!(&lit.lit, Lit::Str(value) if value.value() == expected))
}
fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(lit) if lit.attrs.is_empty() && matches!(&lit.lit, Lit::Bool(value) if value.value == expected))
}
fn check_int(expr: &Expr, expected: i64) -> bool {
    matches!(expr, Expr::Lit(lit) if lit.attrs.is_empty() && matches!(&lit.lit, Lit::Int(value) if value.base10_parse::<i64>().ok() == Some(expected)))
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
    let Expr::Array(array) = args[0] else {
        return false;
    };
    array.elems.len() == expected.len()
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(item, path)| is_path(item, path))
}
fn check_cost(expr: &Expr) -> bool {
    let Some(args) = call_args(expr, "ManaCost", "with_colors") else {
        return false;
    };
    if args.len() != 2 || !check_int(args[0], 4) {
        return false;
    }
    matches!(args[1], Expr::Array(array) if array.elems.len() == 2
        && is_path(&array.elems[0], &["Color", "White"])
        && is_path(&array.elems[1], &["Color", "White"]))
}
fn check_rules(expr: &Expr) -> bool {
    let Expr::Reference(reference) = expr else {
        return false;
    };
    if !reference.attrs.is_empty() || reference.mutability.is_some() {
        return false;
    }
    let Expr::Array(array) = &*reference.expr else {
        return false;
    };
    let expected = [
        "full-rules-fidelity",
        "convoke",
        "base-characteristics",
        "flying",
    ];
    array.elems.len() == expected.len()
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(item, text)| check_string(item, text))
}
fn check_some_integer(expr: &Expr, expected: i64) -> bool {
    matches!(expr, Expr::Call(call) if matches!(&*call.func, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["Some"]))
        && call.args.len() == 1 && check_int(&call.args[0], expected))
}
fn vec_items(expr: &Expr) -> Option<Vec<Expr>> {
    let Expr::Macro(macro_expr) = expr else {
        return None;
    };
    if !macro_expr.attrs.is_empty() || !macro_expr.mac.path.is_ident("vec") {
        return None;
    }
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(macro_expr.mac.tokens.clone())
        .ok()
        .map(IntoIterator::into_iter)
        .map(Iterator::collect)
}
fn check_keywords(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|items| {
        items.len() == 2
            && is_path(&items[0], &["Keyword", "Convoke"])
            && is_path(&items[1], &["Keyword", "Flying"])
    })
}
fn check_empty_vec(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|items| items.is_empty())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    GenericCost,
    NoFlying,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    candidate: ObjectId,
    convokers: Vec<ObjectId>,
    spell_colors: Vec<Color>,
    live_power: i32,
    live_toughness: i32,
    live_flying_at_cast: bool,
    live_flying_at_attack: bool,
    live_flying_at_block: bool,
    ground_blocker: ObjectId,
    block_probe_error: String,
    block_probe_state_unchanged: bool,
    opponent_creature: ObjectId,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CONCLAVE_EQUENAUT_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_CONCLAVE_EQUENAUT_SEED must be an unsigned integer".to_owned()
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
        "no terminal Conclave Equenaut Convoke/Flying trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_conclave_equenaut::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::GenericCost => {
            definition.mana_cost = ManaCost::with_colors(5, [Color::White, Color::White])
        }
        Mutation::NoFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
    }
    let mut game = new_rav_game(definition)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: BIRDS.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF.to_owned(),
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
                card: WATCHWOLF.to_owned(),
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
    for deck in [&deck0, &deck1] {
        if deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Conclave Equenaut fixtures must be shuffled 60-card decks".to_owned());
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 8_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(EquenautPolicy::new(PlayerId(0))),
        Box::new(GroundDefenderPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut live = None;
    let mut live_flying_at_attack = false;
    let mut live_flying_at_block = false;
    let mut probe: Option<(ObjectId, String, bool)> = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            let recent: Vec<_> = game
                .event_log
                .iter()
                .rev()
                .filter(|event| {
                    matches!(
                        event,
                        GameEvent::CardMoved { .. }
                            | GameEvent::SpellCast { .. }
                            | GameEvent::SpellManaPaid { .. }
                            | GameEvent::SpellResolved { .. }
                            | GameEvent::AttackersDeclared { .. }
                            | GameEvent::BlockersDeclared { .. }
                            | GameEvent::GameEnded { .. }
                    )
                })
                .take(30)
                .cloned()
                .collect();
            let spells: Vec<_> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::SpellCast { player, card } => Some((
                        player.0,
                        game.card_definition(*card).map(|definition| definition.id),
                    )),
                    _ => None,
                })
                .collect();
            let battlefield: Vec<_> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::CardMoved {
                        card,
                        to: Zone::Battlefield,
                    } => Some(game.card_definition(*card).map(|definition| definition.id)),
                    _ => None,
                })
                .collect();
            let p0 = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            let p0_hand: Vec<_> = p0.hand.iter().map(|card| card.definition).collect();
            let p0_field: Vec<_> = p0
                .own_battlefield
                .iter()
                .map(|card| (card.definition, card.tapped))
                .collect();
            let p0_pool = format!("{:?}", p0.mana_pool);
            return Err(format!(
                "Conclave Equenaut match hit a bounded game limit at turn {}; p0hand={p0_hand:?}; p0field={p0_field:?}; p0pool={p0_pool}; spells={spells:?}; battlefield_entries={battlefield:?}; recent_relevant_events={recent:?}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
        {
            live_flying_at_attack |= view.own_battlefield.iter().any(|card| {
                card.definition == Some(CARD_ID) && card.keywords.contains(&Keyword::Flying)
            });
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player != player
            && !view.blockers_declared
        {
            let attacker = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let ground = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF) && card.can_block);
            if let (Some(attacker), Some(ground)) = (attacker, ground) {
                live_flying_at_block |= attacker.keywords.contains(&Keyword::Flying);
                if probe.is_none() {
                    let before_events = game.event_log.len();
                    let before_view = view.clone();
                    let attempted_block = PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: ground.id,
                        }],
                    };
                    match game.submit_policy_move(
                        player,
                        "rav.conclave-equenaut-flying-legality-probe.v1",
                        attempted_block,
                    ) {
                        Ok(()) => return Err(
                            "a ground Watchwolf was allowed to block a flying Conclave Equenaut"
                                .to_owned(),
                        ),
                        Err(error) => {
                            let message = error.to_string();
                            if !message
                                .contains("flying attacker can be blocked only by flying or reach")
                            {
                                return Err(format!(
                                    "unexpected Flying legality-probe rejection: {message}"
                                ));
                            }
                            let after_view =
                                game.view_for_player(player).map_err(|e| e.to_string())?;
                            let unchanged =
                                game.event_log.len() == before_events && after_view == before_view;
                            if !unchanged {
                                return Err(
                                    "rejected Flying legality probe changed the live match"
                                        .to_owned(),
                                );
                            }
                            probe = Some((ground.id, message, unchanged));
                        }
                    }
                    // The deliberately rejected legality probe is separate from
                    // the scripted policy's accepted-move accounting.
                    continue;
                }
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Conclave Equenaut policy seat missing")?;
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
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        if live.is_none() {
            if let Some(candidate) = game.event_log.iter().find_map(|event| match event {
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
            }) {
                if game.zone_of(candidate) == Some(Zone::Battlefield) {
                    let characteristics =
                        game.characteristics(candidate).map_err(|e| e.to_string())?;
                    live = Some((
                        candidate,
                        characteristics.power.unwrap_or_default(),
                        characteristics.toughness.unwrap_or_default(),
                        characteristics.keywords.contains(&Keyword::Flying),
                    ));
                }
            }
        }
    }
    let (candidate, live_power, live_toughness, live_flying_at_cast) =
        live.ok_or("Conclave Equenaut never resolved onto the battlefield")?;
    let (ground_blocker, block_probe_error, block_probe_state_unchanged) =
        probe.ok_or("Flying legality probe was not reached")?;
    let convokers: Vec<ObjectId> = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } => Some(*creature),
            _ => None,
        })
        .take(4)
        .collect();
    let spell_colors = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == candidate => Some(colors.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let opponent_creature = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == WATCHWOLF) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .ok_or("opponent Watchwolf was not cast to exercise Flying evasion")?;
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
        candidate,
        convokers,
        spell_colors,
        live_power,
        live_toughness,
        live_flying_at_cast,
        live_flying_at_attack,
        live_flying_at_block,
        ground_blocker,
        block_probe_error,
        block_probe_state_unchanged,
        opponent_creature,
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Conclave Equenaut missing from RAV definition registry")?;
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
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.validate_invariants().map_err(|e| e.to_string())?;
    Ok(game)
}

struct EquenautPolicy {
    player: PlayerId,
}
impl EquenautPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let plains = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            })
            .count();
        let forests = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
            .count();
        let preferred = if plains < 2 {
            Color::White
        } else if forests < 4 {
            Color::Green
        } else if plains <= forests {
            Color::White
        } else {
            Color::Green
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
    fn make_mana(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
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
    fn cast_convoker(&self, view: &GameView) -> Option<PolicyAction> {
        if view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(BIRDS) || card.definition == Some(WATCHWOLF))
            .count()
            >= 4
        {
            return None;
        }
        let creature = view
            .hand
            .iter()
            .find(|card| card.definition == Some(BIRDS))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF))
            })?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return self.make_mana(view, Color::Green);
        }
        if creature.definition == Some(WATCHWOLF) && view.mana_pool.amount(Color::White) == 0 {
            return self.make_mana(view, Color::White);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: creature.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }
    fn cast_equenaut(&self, view: &GameView) -> Option<PolicyAction> {
        if view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
        {
            return None;
        }
        let creatures: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                (card.definition == Some(BIRDS) || card.definition == Some(WATCHWOLF))
                    && !card.tapped
            })
            .take(4)
            .collect();
        if creatures.len() != 4 {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::White) < 2 {
            return self.make_mana(view, Color::White);
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: creatures
                    .into_iter()
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
impl CodePolicy for EquenautPolicy {
    fn id(&self) -> &'static str {
        "rav.conclave-equenaut-convoke-flight.v1"
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
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if let Some(action) = self.cast_convoker(view) {
            return action;
        }
        if let Some(action) = self.cast_equenaut(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct GroundDefenderPolicy {
    player: PlayerId,
}
impl GroundDefenderPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for GroundDefenderPolicy {
    fn id(&self) -> &'static str {
        "rav.ground-watchwolf-no-flying-block.v1"
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let white = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::White)
                })
                .count();
            let green = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Green)
                })
                .count();
            let preferred = if white <= green {
                Color::White
            } else {
                Color::Green
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&preferred)
                })
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !view
            .opponent_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
            || view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF))
        {
            return PolicyAction::PassPriority;
        }
        let watchwolf = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF));
        if let Some(card) = watchwolf {
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
            if view.mana_pool.amount(Color::White) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    !card.tapped
                        && card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::White)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::White,
                    };
                }
            }
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
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

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err("Conclave Equenaut game did not finish with a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("scripted Conclave Equenaut policy contains rejected moves".to_owned());
    }
    if captured.convokers.len() != 4
        || captured
            .convokers
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != 4
    {
        return Err(
            "Conclave Equenaut did not pay exactly four distinct generic mana with Convoke"
                .to_owned(),
        );
    }
    if captured.spell_colors != [Color::White, Color::White] {
        return Err(format!(
            "Conclave Equenaut white symbols were not paid exactly: {:?}",
            captured.spell_colors
        ));
    }
    if (captured.live_power, captured.live_toughness) != (3, 3)
        || !captured.live_flying_at_cast
        || !captured.live_flying_at_attack
        || !captured.live_flying_at_block
    {
        return Err("Conclave Equenaut live 3/3/Flying characteristics were not present at cast, attack, and block".to_owned());
    }
    if !captured.block_probe_state_unchanged
        || !captured
            .block_probe_error
            .contains("flying attacker can be blocked only by flying or reach")
    {
        return Err("ground blocker was not rejected by the live Flying legality rule".to_owned());
    }
    let convoke_indices: Vec<usize> = captured
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } if captured.convokers.contains(creature) => Some(index),
            _ => None,
        })
        .collect();
    if convoke_indices.len() != 4 {
        return Err("typed Convoke event count differs from four generic contributions".to_owned());
    }
    let paid = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == captured.candidate && colors == &[Color::White, Color::White])).ok_or("candidate SpellManaPaid does not contain exactly two white mana")?;
    let cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.candidate)).ok_or("candidate SpellCast event missing")?;
    let resolved = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.candidate),
    )
    .ok_or("candidate SpellResolved event missing")?;
    let entered = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == captured.candidate)).ok_or("candidate Battlefield entry event missing")?;
    if convoke_indices.iter().any(|index| *index >= paid)
        || !(paid < cast && cast < resolved && resolved < entered)
    {
        return Err(
            "Conclave Equenaut ordered generic payment/cast/resolution/entry events differ"
                .to_owned(),
        );
    }
    let opponent_paid = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == captured.opponent_creature && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))
    })
    .ok_or("opponent Watchwolf mana payment event missing")?;
    let opponent_cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == captured.opponent_creature)).ok_or("opponent ground blocker cast event missing")?;
    let opponent_resolved = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.opponent_creature)).ok_or("opponent Watchwolf did not resolve")?;
    let opponent_entry = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == captured.opponent_creature)).ok_or("opponent Watchwolf did not enter the battlefield")?;
    let attack = first_event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&captured.candidate))).ok_or("Conclave Equenaut did not attack")?;
    let blockers = first_event_after(&captured.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("ground Watchwolf did not make the legal empty block declaration against Flying")?;
    let damage = first_event_after(&captured.events, blockers, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == captured.candidate)).ok_or("Conclave Equenaut did not deal three unblocked combat damage")?;
    let ended = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("full game terminal event missing")?;
    if !(entered < opponent_paid
        && opponent_paid < opponent_cast
        && opponent_cast < opponent_resolved
        && opponent_resolved < opponent_entry
        && opponent_entry < attack
        && attack < blockers
        && blockers < damage
        && damage < ended)
    {
        return Err("candidate and combat event provenance is out of order".to_owned());
    }
    let mut selected = BTreeSet::new();
    selected.extend(convoke_indices);
    selected.extend([
        paid,
        cast,
        resolved,
        entered,
        opponent_paid,
        opponent_cast,
        opponent_resolved,
        opponent_entry,
        attack,
        blockers,
        damage,
        ended,
    ]);
    let projected = project_selected(&captured.events, &selected)?;
    Ok(json!({
        "trace_id": "rav-conclave-equenaut-generic-convoke-flying-evasion",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "rejected_policy_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": selected.len(),
        "legality_probe": {
            "attacker": captured.candidate.0,
            "ground_blocker": captured.ground_blocker.0,
            "attacker_live_flying": captured.live_flying_at_block,
            "ground_blocker_can_block_generally": true,
            "rejected_by_engine": true,
            "rejection": captured.block_probe_error,
            "match_state_unchanged": captured.block_probe_state_unchanged
        },
        "mechanics": {
            "cast_from_shuffled_library": true,
            "mana_cost": "{4}{W}{W}",
            "generic_paid_by_convoke": 4,
            "convoke_creatures": captured.convokers.iter().map(|card| card.0).collect::<Vec<_>>(),
            "convoke_contribution": "Generic",
            "colored_symbols_paid": ["White", "White"],
            "live_power_toughness": [captured.live_power, captured.live_toughness],
            "live_flying_at_cast": captured.live_flying_at_cast,
            "live_flying_at_attack": captured.live_flying_at_attack,
            "live_flying_at_block": captured.live_flying_at_block,
            "ground_blocker": captured.ground_blocker.0,
            "ground_blocker_was_unable_to_block": true,
            "combat_damage_to_opponent": 3
        },
        "events": projected
    }))
}

fn project_selected(
    events: &[GameEvent],
    selected: &BTreeSet<usize>,
) -> Result<Vec<Value>, String> {
    let projected = project_events(events);
    let mut output = Vec::new();
    for index in selected {
        let record = projected
            .get(*index)
            .ok_or("projected event index missing")?;
        let mut value = serde_json::to_value(record).map_err(|error| error.to_string())?;
        if let GameEvent::ConvokeUsed {
            player,
            creature,
            contribution: None,
        } = events.get(*index).ok_or("raw event index missing")?
        {
            value["facts"] =
                json!({"seats": [player.0], "objects": [creature.0], "contribution": "Generic"});
        }
        output.push(value);
    }
    Ok(output)
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
        && left.candidate == right.candidate
        && left.convokers == right.convokers
        && left.spell_colors == right.spell_colors
        && left.live_power == right.live_power
        && left.live_toughness == right.live_toughness
        && left.live_flying_at_cast == right.live_flying_at_cast
        && left.live_flying_at_attack == right.live_flying_at_attack
        && left.live_flying_at_block == right.live_flying_at_block
        && left.ground_blocker == right.ground_blocker
        && left.block_probe_error == right.block_probe_error
        && left.block_probe_state_unchanged == right.block_probe_state_unchanged
        && left.opponent_creature == right.opponent_creature
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 9;
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
