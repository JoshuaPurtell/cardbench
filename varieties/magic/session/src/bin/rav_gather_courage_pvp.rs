//! Full-game RAV Gather Courage authority for Convoke and combat-relevant pump.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, ConvokeContribution, ConvokePayment, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ObjectId, PlayerId,
    PolicyAction, Step, Target, Zone,
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
const CARD_ID: &str = "RAV-GATHER-COURAGE";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/gather_courage.rs";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const CONVOKE_SOURCE: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/gather_courage.rs"]
mod candidate_gather_courage;

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
            eprintln!("Gather Courage PvP authority failed: {error}");
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
        return Err("Gather Courage shuffled full-game replay was not deterministic".to_owned());
    }
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events || captured.winner != replay.winner {
        return Err("Gather Courage full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "plus_two_pump_mutation_detected_in_game": mutation_detected(seed, Mutation::PlusOnePump),
        "convoke_keyword_mutation_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "green_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["plus_two_pump_mutation_detected_in_game"] != true
        || mutation_sensitivity["convoke_keyword_mutation_detected_in_game"] != true
        || mutation_sensitivity["green_cost_mutation_detected_in_game"] != true
    {
        return Err("a Gather Courage gameplay mutation escaped the trace oracle".to_owned());
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
        "card": {"collector_number": 165, "definition_id": CARD_ID, "name": "Gather Courage"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "702.50 Convoke",
                "601.2 Casting a spell and paying costs",
                "608.2-608.3 Resolving a spell and creating a continuous effect",
                "613.4 Layer 7 power and toughness modifiers",
                "510 Combat damage and state-based actions"
            ],
            "source_contract": "RAV Gather Courage identity, green Convoke, and a target creature's +2/+2 until end of turn",
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
    let definition = candidate_gather_courage::definition();
    if definition.id != CARD_ID
        || definition.name != "Gather Courage"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects
            != [Effect::ModifyTargetPtUntilEndOfTurn {
                power: 2,
                toughness: 2,
            }]
    {
        return Err("Gather Courage typed definition differs from the feature contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Gather Courage source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Gather Courage source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Gather Courage source has unsupported module items".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|p| p.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Gather Courage source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) if definition.is_none() => definition = Some(function),
            _ => return Err("Gather Courage source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected {
        return Err("Gather Courage imports differ from the source contract".to_owned());
    }
    let function = definition.ok_or("Gather Courage definition() is missing")?;
    let value = direct_struct_function(function, "CardDefinition")?;
    let fields = literal_fields(value)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Gather Courage")
        || !check_string(fields["set_code"], "RAV")
        || !check_green_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Green"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Instant"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "convoke",
                "targeted-layer-7-modifier",
            ],
        )
        || !check_none(fields["power"])
        || !check_none(fields["toughness"])
        || !check_vec_path(fields["keywords"], &["Keyword", "Convoke"])
        || !check_pump_effect(fields["effects"], 2, 2)
    {
        return Err("Gather Courage definition is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn direct_struct_function<'a>(
    function: &'a syn::ItemFn,
    return_type: &str,
) -> Result<&'a syn::ExprStruct, String> {
    let attrs_ok = function.attrs.iter().all(|a| {
        is_doc_attribute(a) || a.path().is_ident("must_use") && matches!(a.meta, Meta::Path(_))
    });
    let crate_visibility =
        matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"));
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
        return Err("Gather Courage definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Gather Courage definition() must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Gather Courage has the wrong literal type".to_owned());
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
fn is_doc_attribute(a: &syn::Attribute) -> bool {
    a.path().is_ident("doc")
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(p) => {
            let mut next = prefix.to_vec();
            next.push(p.ident.to_string());
            flatten_use_tree(&next, &p.tree, paths)
        }
        UseTree::Name(n) => {
            let mut path = prefix.to_vec();
            path.push(n.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(g) => g
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
            .all(|(s, e)| s.ident == *e && matches!(&s.arguments, syn::PathArguments::None))
}
fn path_is_type(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(p) if p.qself.is_none() && path_is(&p.path, &[expected]))
}
fn call_expr<'a>(expr: &'a Expr, owner: &str, method: &str) -> Option<Vec<&'a Expr>> {
    let Expr::Call(c) = expr else {
        return None;
    };
    let Expr::Path(p) = &*c.func else {
        return None;
    };
    if p.qself.is_some() || !path_is(&p.path, &[owner, method]) || !c.attrs.is_empty() {
        return None;
    }
    Some(c.args.iter().collect())
}
fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(v) if v.attrs.is_empty() && matches!(&v.lit, Lit::Str(s) if s.value() == expected))
}
fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(v) if v.attrs.is_empty() && matches!(&v.lit, Lit::Bool(b) if b.value == expected))
}
fn check_none(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(p) if p.qself.is_none() && path_is(&p.path, &["None"]))
}
fn check_int(expr: &Expr, expected: i64) -> bool {
    matches!(expr, Expr::Lit(v) if v.attrs.is_empty() && matches!(&v.lit, Lit::Int(i) if i.base10_parse::<i64>().ok() == Some(expected)))
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
        && array
            .elems
            .iter()
            .zip(expected)
            .all(|(v, p)| matches!(v, Expr::Path(x) if x.qself.is_none() && path_is(&x.path, p)))
}
fn check_green_cost(expr: &Expr) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_colors") else {
        return false;
    };
    args.len() == 2
        && check_int(args[0], 0)
        && matches!(args[1], Expr::Array(a) if a.elems.len() == 1
        && matches!(&a.elems[0], Expr::Path(p) if p.qself.is_none() && path_is(&p.path, &["Color", "Green"])))
}
fn check_rules(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Reference(r) = expr else {
        return false;
    };
    if !r.attrs.is_empty() || r.mutability.is_some() {
        return false;
    }
    let Expr::Array(a) = &*r.expr else {
        return false;
    };
    a.elems.len() == expected.len()
        && a.elems
            .iter()
            .zip(expected)
            .all(|(v, s)| check_string(v, s))
}
fn vec_items(expr: &Expr) -> Option<Vec<Expr>> {
    let Expr::Macro(m) = expr else {
        return None;
    };
    if !m.attrs.is_empty() || !m.mac.path.is_ident("vec") {
        return None;
    }
    syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
        .parse2(m.mac.tokens.clone())
        .ok()
        .map(IntoIterator::into_iter)
        .map(Iterator::collect)
}
fn check_vec_path(expr: &Expr, path: &[&str]) -> bool {
    vec_items(expr).is_some_and(|items| {
        items.len() == 1
            && matches!(&items[0], Expr::Path(p) if p.qself.is_none() && path_is(&p.path, path))
    })
}
fn check_pump_effect(expr: &Expr, power: i64, toughness: i64) -> bool {
    let Some(items) = vec_items(expr) else {
        return false;
    };
    if items.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &items[0] else {
        return false;
    };
    if !path_is(&effect.path, &["Effect", "ModifyTargetPtUntilEndOfTurn"])
        || effect.rest.is_some()
        || effect.fields.len() != 2
    {
        return false;
    }
    let mut p = false;
    let mut t = false;
    for field in &effect.fields {
        let syn::Member::Named(name) = &field.member else {
            return false;
        };
        if name == "power" {
            p = check_int(&field.expr, power);
        } else if name == "toughness" {
            t = check_int(&field.expr, toughness);
        } else {
            return false;
        }
    }
    p && t
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    PlusOnePump,
    NoConvoke,
    GenericCost,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    spell: ObjectId,
    target: ObjectId,
    convoker: ObjectId,
    blocker: ObjectId,
    resolved_cost: ManaCost,
    resolution_pt: (i32, i32),
    convoker_was_tapped: bool,
    target_survived_combat: bool,
    blocker_died_in_combat: bool,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GATHER_COURAGE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GATHER_COURAGE_SEED must be an unsigned integer".to_owned())?;
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
        "no terminal Gather Courage Convoke-combat trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_gather_courage::definition();
    match mutation {
        Mutation::None => {}
        Mutation::PlusOnePump => {
            definition.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }]
        }
        Mutation::NoConvoke => definition.keywords.clear(),
        Mutation::GenericCost => definition.mana_cost = ManaCost::new(1),
    }
    let mut game = new_rav_game(definition)?;
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
                card: CONVOKE_SOURCE.to_owned(),
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
    if deck0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || deck1
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Gather Courage fixtures must be shuffled 60-card decks".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
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
        Box::new(GatherPolicy::new(PlayerId(0))),
        Box::new(WatchwolfPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut target_state = None;
    let mut convoker_state = None;
    let mut combat_state = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Gather Courage match hit a bounded game limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Gather Courage policy seat missing")?;
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

        if target_state.is_none() {
            if let Some((spell, target)) = game.event_log.iter().find_map(|event| match event {
                GameEvent::ContinuousEffectCreated { source, target, .. }
                    if game.card_definition(*source).is_ok_and(|d| d.id == CARD_ID) =>
                {
                    Some((*source, *target))
                }
                _ => None,
            }) {
                let characteristics = game.characteristics(target).map_err(|e| e.to_string())?;
                if let (Some(power), Some(toughness)) =
                    (characteristics.power, characteristics.toughness)
                {
                    target_state = Some((spell, target, power, toughness));
                }
            }
        }
        if mutation == Mutation::PlusOnePump
            && target_state.is_some_and(|(_, _, power, toughness)| (power, toughness) != (5, 5))
        {
            return Err(
                "Gather Courage semantic mutation did not create the required 5/5".to_owned(),
            );
        }
        if mutation == Mutation::GenericCost
            && game.event_log.iter().any(|event| {
                matches!(event,
                GameEvent::SpellCast { player: PlayerId(0), card }
                    if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID))
            })
        {
            return Err(
                "Gather Courage cost mutation changed the cast identity's green symbol".to_owned(),
            );
        }
        if convoker_state.is_none() {
            if let Some((spell, convoker)) =
                game.event_log
                    .iter()
                    .enumerate()
                    .find_map(|(index, event)| {
                        let GameEvent::ConvokeUsed {
                            player: PlayerId(0),
                            creature,
                            contribution: Some(Color::Green),
                        } = event
                        else {
                            return None;
                        };
                        let spell =
                            game.event_log[index..]
                                .iter()
                                .find_map(|later| match later {
                                    GameEvent::SpellCast {
                                        player: PlayerId(0),
                                        card,
                                    } if game
                                        .card_definition(*card)
                                        .is_ok_and(|d| d.id == CARD_ID) =>
                                    {
                                        Some(*card)
                                    }
                                    _ => None,
                                })?;
                        Some((spell, *creature))
                    })
            {
                let tapped = game.object(convoker).map_err(|e| e.to_string())?.tapped;
                convoker_state = Some((spell, convoker, tapped));
            }
        }
        if combat_state.is_none() {
            if let Some((_, target, _, _)) = target_state {
                let target_attacked = game.event_log.iter().any(|event| matches!(event,
                    GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target)));
                let blocked_combat =
                    game.event_log
                        .iter()
                        .enumerate()
                        .find_map(|(block_index, event)| {
                            let GameEvent::BlockersDeclared {
                                player: PlayerId(1),
                                assignments,
                            } = event
                            else {
                                return None;
                            };
                            let blocker = assignments.iter().find_map(|(attacker, blocker)| {
                                (*attacker == target).then_some(*blocker)
                            })?;
                            let end_index = game
                                .event_log
                                .iter()
                                .enumerate()
                                .skip(block_index + 1)
                                .find_map(|(index, later)| {
                                    matches!(
                                        later,
                                        GameEvent::StepBegan {
                                            step: Step::EndOfCombat,
                                            ..
                                        }
                                    )
                                    .then_some(index)
                                })?;
                            Some((blocker, end_index))
                        });
                if target_attacked && let Some((blocker, _end_index)) = blocked_combat {
                    combat_state = Some((
                        target,
                        blocker,
                        game.zone_of(target) == Some(Zone::Battlefield),
                        game.zone_of(blocker) == Some(Zone::Graveyard),
                    ));
                }
            }
        }
    }
    let (spell, target, power, toughness) = target_state.ok_or_else(|| {
        format!(
            "Gather Courage never created a target-bound pump; relevant events: {:?}",
            game.event_log
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::SpellCast { .. }
                        | GameEvent::ConvokeUsed { .. }
                        | GameEvent::SpellResolved { .. }
                        | GameEvent::ContinuousEffectCreated { .. }
                        | GameEvent::AttackersDeclared { .. }
                        | GameEvent::BlockersDeclared { .. }
                ))
                .collect::<Vec<_>>()
        )
    })?;
    let (convoke_spell, convoker, convoker_was_tapped) =
        convoker_state.ok_or("Gather Courage never paid colored mana through Convoke")?;
    let (combat_target, blocker, target_survived_combat, blocker_died_in_combat) =
        combat_state.ok_or("Gather Courage target did not complete its blocked combat")?;
    if spell != convoke_spell || target != combat_target {
        return Err(
            "Gather Courage payment, pump, and combat target provenance differed".to_owned(),
        );
    }
    let winner = game.winner().map(|player| player.0);
    let resolved_cost = game
        .card_definition(spell)
        .map_err(|e| e.to_string())?
        .mana_cost
        .clone();
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
        spell,
        target,
        convoker,
        blocker,
        resolved_cost,
        resolution_pt: (power, toughness),
        convoker_was_tapped,
        target_survived_combat,
        blocker_died_in_combat,
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Gather Courage missing from RAV definition registry")?;
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

struct GatherPolicy {
    player: PlayerId,
    elf_used: bool,
    gather_cast: bool,
}
impl GatherPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_used: false,
            gather_cast: false,
        }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let forest = view
            .own_battlefield
            .iter()
            .filter(|c| c.mana_colors.contains(&Color::Green))
            .count();
        let plains = view
            .own_battlefield
            .iter()
            .filter(|c| c.mana_colors.contains(&Color::White))
            .count();
        let preferred = if forest <= plains {
            Color::Green
        } else {
            Color::White
        };
        view.hand
            .iter()
            .find(|c| c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land))
            })
            .map(|c| PolicyAction::PlayLand { card: c.id })
    }
    fn make_mana(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|c| {
                !c.tapped
                    && c.card_types.contains(&CardType::Land)
                    && c.mana_colors.contains(&color)
            })
            .map(|c| PolicyAction::ActivateManaAbility { land: c.id, color })
    }
    fn cast_gather(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.gather_cast
            || view.step != Step::DeclareAttackers
            || view.active_player != self.player
            || !view.attackers_declared
            || view.stack_depth != 0
            || view.priority != self.player
        {
            return None;
        }
        let attacker = view
            .combat_attackers
            .iter()
            .find(|c| c.definition == Some(WATCHWOLF))?;
        let elf = view
            .own_battlefield
            .iter()
            .find(|c| c.definition == Some(CONVOKE_SOURCE) && !c.tapped)?;
        let card = view.hand.iter().find(|c| c.definition == Some(CARD_ID))?;
        self.gather_cast = true;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![Target::Permanent(attacker.id)],
            convoke: vec![ConvokePayment {
                creature: elf.id,
                contribution: ConvokeContribution::Color(Color::Green),
            }],
            payment_mana_abilities: vec![],
        }))
    }
    fn cast_elf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.elf_used
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|c| c.definition == Some(CONVOKE_SOURCE))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return self.make_mana(view, Color::Green);
        }
        self.elf_used = true;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
    fn cast_wolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view.hand.iter().find(|c| c.definition == Some(WATCHWOLF))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return self.make_mana(view, Color::Green);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.make_mana(view, Color::White);
        }
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
}
impl CodePolicy for GatherPolicy {
    fn id(&self) -> &'static str {
        "rav.gather_courage-convoke-blocked-combat.v1"
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
                    .filter(|c| {
                        c.definition == Some(WATCHWOLF)
                            && c.can_attack
                            && (self.gather_cast
                                || (view
                                    .hand
                                    .iter()
                                    .any(|card| card.definition == Some(CARD_ID))
                                    && view.own_battlefield.iter().any(|creature| {
                                        creature.definition == Some(CONVOKE_SOURCE)
                                            && !creature.tapped
                                    })))
                    })
                    .map(|c| c.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignment = view
                .combat_attackers
                .iter()
                .find(|a| a.definition == Some(WATCHWOLF))
                .and_then(|a| {
                    view.own_battlefield
                        .iter()
                        .find(|b| b.definition == Some(WATCHWOLF) && b.can_block)
                        .map(|b| CombatBlock {
                            attacker: a.id,
                            blocker: b.id,
                        })
                });
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.cast_gather(view) {
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
        if let Some(action) = self.cast_elf(view) {
            return action;
        }
        if let Some(action) = self.cast_wolf(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct WatchwolfPolicy {
    player: PlayerId,
    wolf_cast: bool,
}
impl WatchwolfPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            wolf_cast: false,
        }
    }
}
impl CodePolicy for WatchwolfPolicy {
    fn id(&self) -> &'static str {
        "rav.watchwolf-reserve-blocker.v1"
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
                .find(|a| a.definition == Some(WATCHWOLF))
                .and_then(|a| {
                    view.own_battlefield
                        .iter()
                        .find(|b| b.definition == Some(WATCHWOLF) && b.can_block)
                        .map(|b| CombatBlock {
                            attacker: a.id,
                            blocker: b.id,
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
            let preferred = [Color::Green, Color::White]
                .into_iter()
                .min_by_key(|color| {
                    view.own_battlefield
                        .iter()
                        .filter(|c| c.mana_colors.contains(color))
                        .count()
                })
                .unwrap_or(Color::Green);
            if let Some(land) = view
                .hand
                .iter()
                .find(|c| {
                    c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&preferred)
                })
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|c| c.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.wolf_cast
            && let Some(card) = view.hand.iter().find(|c| c.definition == Some(WATCHWOLF))
        {
            if view.mana_pool.amount(Color::Green) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|c| {
                    !c.tapped
                        && c.card_types.contains(&CardType::Land)
                        && c.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            }
            if view.mana_pool.amount(Color::White) == 0 {
                if let Some(land) = view.own_battlefield.iter().find(|c| {
                    !c.tapped
                        && c.card_types.contains(&CardType::Land)
                        && c.mana_colors.contains(&Color::White)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::White,
                    };
                }
            }
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                self.wolf_cast = true;
                return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err("Gather Courage game did not finish with a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Gather Courage reference match contains rejected policy moves".to_owned());
    }
    if captured.resolved_cost != ManaCost::with_colors(0, [Color::Green]) {
        return Err("Gather Courage no longer has its exact green Convoke cost".to_owned());
    }
    let attack = first_event_index(&captured.events, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&captured.target))).ok_or("pumped Watchwolf did not attack")?;
    let convoke = unique_event_index(&captured.events, |e| matches!(e, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: Some(Color::Green) } if *creature == captured.convoker)).ok_or("Gather Courage missing exact green Convoke receipt")?;
    let cast = unique_event_index(&captured.events, |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.spell)).ok_or("Gather Courage cast receipt missing")?;
    let resolution = unique_event_index(
        &captured.events,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == captured.spell),
    )
    .ok_or("Gather Courage resolution receipt missing")?;
    let spell_graveyard = unique_event_index(&captured.events, |e| {
        matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.spell)
    })
    .ok_or("Gather Courage did not move to its owner's graveyard")?;
    let pump = unique_event_index(&captured.events, |e| matches!(e, GameEvent::ContinuousEffectCreated { source, target, .. } if *source == captured.spell && *target == captured.target)).ok_or("Gather Courage pump was not bound to the selected target")?;
    let blocks = unique_event_index(&captured.events, |e| matches!(e, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(captured.target, captured.blocker)))).ok_or("pumped Watchwolf did not face the real Watchwolf blocker")?;
    let target_hit = unique_event_index(&captured.events, |e| matches!(e, GameEvent::DamageDealtToPermanent { source, permanent, amount: 5 } if *source == captured.target && *permanent == captured.blocker)).ok_or("pumped attacker did not deal five combat damage to its blocker")?;
    let blocker_hit = unique_event_index(&captured.events, |e| matches!(e, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == captured.blocker && *permanent == captured.target)).ok_or("Watchwolf blocker did not deal its three combat damage back")?;
    let end = unique_event_index(&captured.events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Gather Courage shuffled game lacks terminal winner event")?;
    if !(attack < convoke
        && convoke < cast
        && cast < pump
        && pump < resolution
        && resolution < spell_graveyard
        && spell_graveyard < blocks
        && resolution < blocks
        && blocks < target_hit
        && blocks < blocker_hit
        && target_hit < end
        && blocker_hit < end)
    {
        return Err(format!(
            "Gather Courage event ordering/provenance failed: attack={attack}, convoke={convoke}, cast={cast}, pump={pump}, resolve={resolution}, spell_graveyard={spell_graveyard}, blocks={blocks}, damage={target_hit}/{blocker_hit}, end={end}"
        ));
    }
    if captured.resolution_pt != (5, 5)
        || !captured.convoker_was_tapped
        || !captured.target_survived_combat
        || !captured.blocker_died_in_combat
    {
        return Err(format!(
            "Gather Courage live state mismatch: target_pt={:?}, convoker_tapped={}, target_survived={}, blocker_died={}",
            captured.resolution_pt,
            captured.convoker_was_tapped,
            captured.target_survived_combat,
            captured.blocker_died_in_combat
        ));
    }
    let evidence_events: Vec<_> = captured
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => *index == attack && attackers.contains(&captured.target),
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                ..
            } => *creature == captured.convoker,
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                ..
            }
            | GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            }
            | GameEvent::SpellResolved { card } => *card == captured.spell,
            GameEvent::ContinuousEffectCreated { source, target, .. } => {
                *source == captured.spell && *target == captured.target
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments.contains(&(captured.target, captured.blocker)),
            GameEvent::DamageDealtToPermanent {
                source, permanent, ..
            } => {
                (*source == captured.target && *permanent == captured.blocker)
                    || (*source == captured.blocker && *permanent == captured.target)
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => *card == captured.blocker || *card == captured.spell,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .map(|(_, event)| event.clone())
        .collect();
    let projected = project_events(&evidence_events);
    Ok(json!({
        "trace_id": "rav-gather-courage-convoke-blocked-combat",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "rejected_policy_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": evidence_events.len(),
        "mechanics": {
            "cast_from_shuffled_library": true,
            "targeted_attacker": captured.target.0,
            "payment": {"creature": captured.convoker.0, "contribution": "green", "tapped": captured.convoker_was_tapped},
            "spell": captured.spell.0,
            "mana_cost": "{G}",
            "resolution_power_toughness": [captured.resolution_pt.0, captured.resolution_pt.1],
            "blocker": captured.blocker.0,
            "attacker_damage": 5,
            "blocker_damage": 3,
            "target_survived_combat": captured.target_survived_combat,
            "blocker_died_in_combat": captured.blocker_died_in_combat,
        },
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
        .filter_map(|(i, event)| predicate(event).then_some(i));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn first_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(predicate)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 165;
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
