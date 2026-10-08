//! Full-game RAV Rally the Righteous authority for Radiance and combat-relevant pump.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
const CARD_ID: &str = "RAV-RALLY-THE-RIGHTEOUS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/rally_the_righteous.rs";
const BROWN_SCALE: &str = "RAV-GOLGARI-BROWNSCALE";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/rally_the_righteous.rs"]
mod candidate_rally_the_righteous;

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
            eprintln!("Rally the Righteous PvP authority failed: {error}");
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
            "Rally the Righteous shuffled full-game replay was not deterministic".to_owned(),
        );
    }
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events || captured.winner != replay.winner {
        return Err("Rally the Righteous full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "two_power_radiance_combat_mutation_detected_in_game": mutation_detected(seed, Mutation::PlusOnePower),
        "shared_color_and_untap_mutation_detected_in_game": mutation_detected(seed, Mutation::TargetOnly),
        "red_white_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["two_power_radiance_combat_mutation_detected_in_game"] != true
        || mutation_sensitivity["shared_color_and_untap_mutation_detected_in_game"] != true
        || mutation_sensitivity["red_white_cost_mutation_detected_in_game"] != true
    {
        return Err("a Rally the Righteous gameplay mutation escaped the trace oracle".to_owned());
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
        "card": {"collector_number": 222, "definition_id": CARD_ID, "name": "Rally the Righteous"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "207.2c Ability words (Radiance)",
                "601.2 Casting a spell and paying costs",
                "608.2-608.3 Resolving a spell and creating a continuous effect",
                "613.4 Layer 7 power and toughness modifiers",
                "502 Untap step",
                "510 Combat damage",
                "704 State-based actions"
            ],
            "source_contract": "RAV Rally the Righteous identity, {1}{R}{W} instant, and Radiance that untaps and gives target and same-color creatures +2/+0 until end of turn",
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
    let definition = candidate_rally_the_righteous::definition();
    if definition.id != CARD_ID
        || definition.name != "Rally the Righteous"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Red, Color::White])
        || definition.colors != BTreeSet::from([Color::Red, Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.effects
            != [Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }]
    {
        return Err(
            "Rally the Righteous typed definition differs from the feature contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Rally the Righteous source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Rally the Righteous source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Rally the Righteous source has unsupported module items".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
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
                    return Err("Rally the Righteous source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) if definition.is_none() => definition = Some(function),
            _ => {
                return Err(
                    "Rally the Righteous source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if imports != expected {
        return Err("Rally the Righteous imports differ from the source contract".to_owned());
    }
    let function = definition.ok_or("Rally the Righteous definition() is missing")?;
    let value = direct_struct_function(function, "CardDefinition")?;
    let fields = literal_fields(value)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Rally the Righteous")
        || !check_string(fields["set_code"], "RAV")
        || !check_rally_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Red"], &["Color", "White"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Instant"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "radiance",
                "untap",
                "layer-7-modifier",
            ],
        )
        || !check_none(fields["power"])
        || !check_none(fields["toughness"])
        || !check_vec_empty(fields["keywords"])
        || !check_radiance_effect(fields["effects"], 2, 0)
    {
        return Err(
            "Rally the Righteous definition is outside its literal source contract".to_owned(),
        );
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
        return Err("Rally the Righteous definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Rally the Righteous definition() must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Rally the Righteous has the wrong literal type".to_owned());
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
fn check_rally_cost(expr: &Expr) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_colors") else {
        return false;
    };
    args.len() == 2
        && check_int(args[0], 1)
        && matches!(args[1], Expr::Array(a) if a.elems.len() == 2
        && matches!(&a.elems[0], Expr::Path(p) if p.qself.is_none() && path_is(&p.path, &["Color", "Red"]))
        && matches!(&a.elems[1], Expr::Path(p) if p.qself.is_none() && path_is(&p.path, &["Color", "White"])))
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
fn check_vec_empty(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|items| items.is_empty())
}
fn check_radiance_effect(expr: &Expr, power: i64, toughness: i64) -> bool {
    let Some(items) = vec_items(expr) else {
        return false;
    };
    if items.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &items[0] else {
        return false;
    };
    if !path_is(
        &effect.path,
        &["Effect", "RadianceUntapAndModifyUntilEndOfTurn"],
    ) || effect.rest.is_some()
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
    PlusOnePower,
    TargetOnly,
    WrongCost,
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
    attackers: Vec<ObjectId>,
    radiance_recipients: Vec<ObjectId>,
    untapped: Vec<ObjectId>,
    live_snapshots: BTreeMap<ObjectId, (i32, i32, bool)>,
    mana_paid: Vec<Color>,
    spell_cost: ManaCost,
    blocker: ObjectId,
    target_zone: Option<Zone>,
    life: [i64; 2],
    target_stack_verified: bool,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_RALLY_THE_RIGHTEOUS_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_RALLY_THE_RIGHTEOUS_SEED must be an unsigned integer".to_owned()
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
        "no Rally the Righteous Radiance combat trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_rally_the_righteous::definition();
    match mutation {
        Mutation::None => {}
        Mutation::PlusOnePower => {
            definition.effects = vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 1,
                toughness: 0,
            }];
        }
        Mutation::TargetOnly => {
            definition.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }];
        }
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::White, Color::White]);
        }
    }
    let mut game = new_rav_game(definition)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-BOROS-SWIFTBLADE".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
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
        mainboard: vec![
            DeckEntry {
                card: BROWN_SCALE.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
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
        return Err("Rally the Righteous fixtures must be shuffled 60-card decks".to_owned());
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
        Box::new(RallyPolicy::new(PlayerId(0))),
        Box::new(BrownscalePolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut rally_spell = None;
    let mut selected_target = None;
    let mut stack_verified = false;
    let mut attack_snapshot = Vec::new();
    let mut radiance_recipients = Vec::new();
    let mut untapped = Vec::new();
    let mut live_snapshots = BTreeMap::new();

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Rally the Righteous match hit a bounded limit at turn {} / {:?}",
                game.turn, game.step
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Rally the Righteous policy seat missing")?;
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
        let rally_cast = match &action {
            PolicyAction::CastWithPayment { request, .. }
                if request.targets.len() == 1
                    && game
                        .card_definition(request.card)
                        .is_ok_and(|card| card.id == CARD_ID) =>
            {
                match request.targets[0] {
                    Target::Permanent(target) => Some((request.card, target)),
                    _ => None,
                }
            }
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Rally the Righteous policy move rejected on turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;

        if attack_snapshot.is_empty() {
            if let Some(attackers) = game.event_log.iter().find_map(|event| match event {
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if attackers.len() >= 2
                    && attackers.iter().all(|id| {
                        game.card_definition(*id)
                            .is_ok_and(|card| card.id == "RAV-BOROS-SWIFTBLADE")
                    }) =>
                {
                    Some(attackers.clone())
                }
                _ => None,
            }) {
                attack_snapshot = attackers;
            }
        }
        if let Some((spell, target)) = rally_cast {
            let stack = game
                .stack
                .last()
                .ok_or("Rally the Righteous did not enter the stack")?;
            if stack.card != spell || stack.targets != [Target::Permanent(target)] {
                return Err(
                    "Rally the Righteous target was not confirmed on its live stack object"
                        .to_owned(),
                );
            }
            rally_spell = Some(spell);
            selected_target = Some(target);
            stack_verified = true;
        }
        if let Some(spell) = rally_spell
            && live_snapshots.is_empty()
            && game
                .event_log
                .iter()
                .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        {
            radiance_recipients = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::ContinuousEffectCreated { source, target, .. }
                        if *source == spell =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
                .collect();
            if let Some(untap_event) = game.event_log.iter().rev().find_map(|event| match event {
                GameEvent::PermanentsUntapped {
                    player: PlayerId(0),
                    cards,
                } => Some(cards.clone()),
                _ => None,
            }) {
                untapped = untap_event;
            }
            for object in &attack_snapshot {
                let characteristics = game
                    .characteristics(*object)
                    .map_err(|error| error.to_string())?;
                let (power, toughness) = (
                    characteristics.power.ok_or("Rally recipient lost power")?,
                    characteristics
                        .toughness
                        .ok_or("Rally recipient lost toughness")?,
                );
                let tapped = game
                    .object(*object)
                    .map_err(|error| error.to_string())?
                    .tapped;
                live_snapshots.insert(*object, (power, toughness, tapped));
            }
        }
    }

    let spell = rally_spell.ok_or("Rally the Righteous was not cast in the full game")?;
    let target = selected_target.ok_or("Rally the Righteous target was not selected")?;
    let blocker = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments
                .iter()
                .find_map(|(attacker, blocker)| (*attacker == target).then_some(*blocker)),
            _ => None,
        })
        .ok_or("Rally the Righteous target did not face the assigned blocker")?;
    let mana_paid = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == spell => Some(colors.clone()),
            _ => None,
        })
        .ok_or("Rally the Righteous has no exact spell payment receipt")?;
    let spell_cost = game
        .card_definition(spell)
        .map_err(|error| error.to_string())?
        .mana_cost
        .clone();
    let target_zone = game.zone_of(target);
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
        spell,
        target,
        attackers: attack_snapshot,
        radiance_recipients,
        untapped,
        live_snapshots,
        mana_paid,
        spell_cost,
        blocker,
        target_zone,
        life: [game.players[0].life, game.players[1].life],
        target_stack_verified: stack_verified,
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Rally the Righteous missing from RAV definition registry")?;
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

struct RallyPolicy {
    player: PlayerId,
    swiftblades_cast: usize,
    rally_cast: bool,
}
impl RallyPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            swiftblades_cast: 0,
            rally_cast: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let red = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Red)
            })
            .count();
        let white = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            })
            .count();
        let desired = if red < 2 {
            Color::Red
        } else if white < 1 {
            Color::White
        } else if red <= white {
            Color::Red
        } else {
            Color::White
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&desired)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn activate_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
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

    fn cast_swiftblade(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.swiftblades_cast >= 2
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-BOROS-SWIFTBLADE"))?;
        if view.mana_pool.amount(Color::Red) == 0 {
            return self.activate_land(view, Color::Red);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return self.activate_land(view, Color::White);
        }
        self.swiftblades_cast += 1;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }

    fn cast_rally(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.rally_cast
            || view.active_player != self.player
            || view.step != Step::DeclareAttackers
            || !view.attackers_declared
            || view.stack_depth != 0
            || view.priority != self.player
        {
            return None;
        }
        let attacker = view
            .combat_attackers
            .iter()
            .filter(|card| card.definition == Some("RAV-BOROS-SWIFTBLADE"))
            .max_by_key(|card| card.id.0)?;
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::Red) < 2 {
            return self.activate_land(view, Color::Red);
        }
        if view.mana_pool.amount(Color::White) < 1 {
            return self.activate_land(view, Color::White);
        }
        self.rally_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(attacker.id)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Red],
                hybrid: vec![],
            },
        })
    }

    fn declared_attackers(&self, view: &GameView) -> Vec<ObjectId> {
        let ready: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some("RAV-BOROS-SWIFTBLADE") && card.can_attack)
            .map(|card| card.id)
            .collect();
        if self.player == PlayerId(0)
            && ready.len() >= 2
            && view.turn >= 4
            && view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(BROWN_SCALE))
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
            && !self.rally_cast
        {
            ready
        } else if self.player == PlayerId(0) && self.rally_cast {
            ready
        } else {
            vec![]
        }
    }
}
impl CodePolicy for RallyPolicy {
    fn id(&self) -> &'static str {
        "rav.rally-the-righteous-radiance-combat.v1"
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
                attackers: self.declared_attackers(view),
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
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.cast_rally(view) {
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
        if let Some(action) = self.cast_swiftblade(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct BrownscalePolicy {
    player: PlayerId,
    brownscale_cast: bool,
}
impl BrownscalePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            brownscale_cast: false,
        }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
    fn make_green(&self, view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color: Color::Green,
            })
    }
}
impl CodePolicy for BrownscalePolicy {
    fn id(&self) -> &'static str {
        "rav.rally-the-righteous-brownscale-opponent.v1"
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
            let attacker = view
                .combat_attackers
                .iter()
                .filter(|card| card.definition == Some("RAV-BOROS-SWIFTBLADE"))
                .max_by_key(|card| card.id.0);
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BROWN_SCALE) && card.can_block);
            let assignments = attacker
                .zip(blocker)
                .map(|(attacker, blocker)| CombatBlock {
                    attacker: attacker.id,
                    blocker: blocker.id,
                })
                .into_iter()
                .collect();
            return PolicyAction::DeclareBlockers { assignments };
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
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.brownscale_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BROWN_SCALE))
        {
            if view.mana_pool.amount(Color::Green) < 3 {
                return self.make_green(view).unwrap_or(PolicyAction::PassPriority);
            }
            self.brownscale_cast = true;
            return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err(
            "Rally the Righteous shuffled PvP game did not end in a seat-zero win".to_owned(),
        );
    }
    if captured.rejected_moves != 0 || captured.life[0] <= 0 || captured.life[1] > 0 {
        return Err(
            "Rally the Righteous match has rejected moves or invalid terminal life totals"
                .to_owned(),
        );
    }
    if captured.spell_cost != ManaCost::with_colors(1, [Color::Red, Color::White]) {
        return Err("Rally the Righteous cost is not exactly {1}{R}{W}".to_owned());
    }
    if captured.mana_paid.len() != 3
        || captured
            .mana_paid
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 2
        || captured
            .mana_paid
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            != 1
    {
        return Err(
            "Rally the Righteous did not pay its red and white symbols plus red generic mana"
                .to_owned(),
        );
    }
    if !captured.target_stack_verified
        || !captured.attackers.contains(&captured.target)
        || captured.attackers.len() != 2
    {
        return Err("Rally the Righteous target was not confirmed on the stack as one of two attacking Swiftblades".to_owned());
    }
    let expected_creatures: BTreeSet<_> = captured.attackers.iter().copied().collect();
    let radiance_creatures: BTreeSet<_> = captured.radiance_recipients.iter().copied().collect();
    let untapped_creatures: BTreeSet<_> = captured.untapped.iter().copied().collect();
    if radiance_creatures != expected_creatures || untapped_creatures != expected_creatures {
        return Err(
            "Rally the Righteous did not create and untap the full same-color attacker set"
                .to_owned(),
        );
    }
    if captured.live_snapshots.len() != 2
        || captured
            .live_snapshots
            .values()
            .any(|snapshot| *snapshot != (3, 2, false))
    {
        return Err(format!(
            "Rally the Righteous live recipient state is not two untapped 3/2 creatures: {:?}",
            captured.live_snapshots
        ));
    }
    if captured.target_zone != Some(Zone::Battlefield) {
        return Err(
            "Rally the Righteous first-strike combat did not preserve its targeted attacker"
                .to_owned(),
        );
    }

    let attack = captured.events.iter().position(|event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if captured.attackers.iter().all(|attacker| attackers.contains(attacker))
    )).ok_or("both Swiftblades did not attack in the Rally combat")?;
    let payment = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.spell
        )
    })
    .ok_or("Rally the Righteous mana payment receipt is missing")?;
    let cast = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.spell
        )
    })
    .ok_or("Rally the Righteous cast receipt is missing")?;
    let target_effect = unique_event_index(&captured.events, |event| matches!(event,
        GameEvent::ContinuousEffectCreated { source, target, .. } if *source == captured.spell && *target == captured.target
    )).ok_or("Rally the Righteous target did not receive its continuous effect")?;
    let peer = captured
        .attackers
        .iter()
        .copied()
        .find(|attacker| *attacker != captured.target)
        .ok_or("Rally peer attacker is missing")?;
    let peer_effect = unique_event_index(&captured.events, |event| matches!(event,
        GameEvent::ContinuousEffectCreated { source, target, .. } if *source == captured.spell && *target == peer
    )).ok_or("Rally Radiance did not reach the second shared-color creature")?;
    let resolved = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == captured.spell
        )
    })
    .ok_or("Rally the Righteous did not resolve")?;
    let untap = captured
        .events
        .iter()
        .enumerate()
        .skip(peer_effect + 1)
        .take(resolved.saturating_sub(peer_effect + 1))
        .find_map(|(index, event)| matches!(event,
            GameEvent::PermanentsUntapped { player: PlayerId(0), cards } if captured.attackers.iter().all(|attacker| cards.contains(attacker))
        ).then_some(index))
        .ok_or("Rally Radiance did not untap both tapped attackers")?;
    let spell_move = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.spell
        )
    })
    .ok_or("Rally the Righteous did not move to its owner's graveyard")?;
    let blocks = unique_event_index(&captured.events, |event| matches!(event,
        GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(captured.target, captured.blocker))
    )).ok_or("Golgari Brownscale did not block the targeted Swiftblade")?;
    let first_strike_step = captured
        .events
        .iter()
        .enumerate()
        .skip(blocks + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    active_player: PlayerId(0),
                    step: Step::FirstStrikeCombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("Rally combat did not reach first-strike damage")?;
    let blocker_damage = unique_event_index(&captured.events, |event| matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == captured.target && *permanent == captured.blocker
    )).ok_or("3-power targeted Swiftblade did not deal lethal first-strike damage")?;
    let sba = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::StateBasedAction { card, .. } if *card == captured.blocker
        )
    })
    .ok_or("lethal first-strike damage did not cause the blocker state-based action")?;
    let blocker_move = unique_event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.blocker
        )
    })
    .ok_or("Golgari Brownscale did not move to its owner's graveyard")?;
    let normal_step = captured
        .events
        .iter()
        .enumerate()
        .skip(first_strike_step + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    active_player: PlayerId(0),
                    step: Step::CombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("Rally combat did not proceed to normal combat damage")?;
    let first_strike_player_damage = captured
        .events
        .iter()
        .enumerate()
        .skip(first_strike_step + 1)
        .take(normal_step.saturating_sub(first_strike_step + 1))
        .find_map(|(index, event)| matches!(event,
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == peer
        ).then_some(index))
        .ok_or("the second buffed Swiftblade did not deal three first-strike damage to the opponent")?;
    let normal_player_damage = captured
        .events
        .iter()
        .enumerate()
        .skip(normal_step + 1)
        .find_map(|(index, event)| {
            matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == peer
    ).then_some(index)
        })
        .ok_or("the second buffed Swiftblade did not deal normal combat damage")?;
    let end = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Rally the Righteous full-game win receipt is missing")?;
    if !(attack < payment
        && payment < cast
        && cast < target_effect
        && target_effect < peer_effect
        && peer_effect < untap
        && untap < resolved
        && resolved < spell_move
        && spell_move < blocks
        && blocks < first_strike_step
        && first_strike_step < blocker_damage
        && blocker_damage < sba
        && sba < blocker_move
        && blocker_move < normal_step
        && first_strike_step < first_strike_player_damage
        && first_strike_player_damage < normal_step
        && normal_step < normal_player_damage
        && normal_player_damage < end)
    {
        return Err("Rally the Righteous cast, Radiance, untap, and First Strike combat events are out of order".to_owned());
    }

    let chosen = [
        attack,
        payment,
        cast,
        target_effect,
        peer_effect,
        untap,
        resolved,
        spell_move,
        blocks,
        first_strike_step,
        blocker_damage,
        sba,
        blocker_move,
        first_strike_player_damage,
        normal_step,
        normal_player_damage,
        end,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let events: Vec<_> = chosen
        .iter()
        .map(|index| captured.events[*index].clone())
        .collect();
    let projected = project_events(&events);
    let snapshots: Vec<_> = captured
        .live_snapshots
        .iter()
        .map(|(object, (power, toughness, tapped))| {
            json!({
                "object": object.0, "power": power, "toughness": toughness, "tapped": tapped
            })
        })
        .collect();
    Ok(json!({
        "trace_id": "rav-rally-the-righteous-radiance-first-strike-combat",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": events.len(),
        "spell": {"object": captured.spell.0, "mana_cost": "{1}{R}{W}", "mana_paid": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "target_selection": {"spell_object": captured.spell.0, "target": captured.target.0, "target_definition": "RAV-BOROS-SWIFTBLADE", "stack_object_verified": captured.target_stack_verified},
        "radiance": {"target": captured.target.0, "affected_creatures": captured.radiance_recipients.iter().map(|object| object.0).collect::<Vec<_>>(), "untapped_creatures": captured.untapped.iter().map(|object| object.0).collect::<Vec<_>>(), "live_snapshots": snapshots, "power_bonus": 2, "toughness_bonus": 0},
        "combat": {"attackers": captured.attackers.iter().map(|object| object.0).collect::<Vec<_>>(), "blocker": captured.blocker.0, "target_first_strike_damage": 3, "blocker_died_before_normal_damage": true, "peer_first_strike_damage_to_player": 3, "peer_normal_damage_to_player": 3},
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
        .filter_map(|(i, event)| predicate(event).then_some(i));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
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
    let passed = card.collector_number == 222;
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
