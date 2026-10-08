//! Full-game RAV Fiery Conclusion authority for cost-paid combat removal.

use cardbench_magic_engine::{
    AdditionalSpellCost, CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, TargetRequirement, Zone,
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
const CARD_ID: &str = "RAV-FIERY-CONCLUSION";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/fiery_conclusion.rs";
const ELF: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WOLFWOLF: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/fiery_conclusion.rs"]
mod candidate_fiery_conclusion;

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
            eprintln!("Fiery Conclusion PvP authority failed: {error}");
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
        return Err("Fiery Conclusion shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "four_damage_mutation_detected_in_game": mutation_detected(seed, Mutation::DamageFour),
        "sacrifice_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::CostRemoved),
        "red_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "four_damage_mutation_detected_in_game",
        "sacrifice_cost_mutation_detected_in_game",
        "red_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Fiery Conclusion gameplay mutation escaped the oracle: {key}"
            ));
        }
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
        "card": {"collector_number": 122, "definition_id": CARD_ID, "name": "Fiery Conclusion"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["510 Combat damage", "601.2 Casting spells and paying costs", "608.2-608.3 Resolving spells", "701.21 Sacrifice", "704 State-based actions"],
            "source_contract": "RAV Fiery Conclusion identity, red instant cost, mandatory controlled-creature sacrifice, and five damage to a target creature",
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
    let definition = candidate_fiery_conclusion::definition();
    if definition.id != CARD_ID
        || definition.name != "Fiery Conclusion"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::DealDamage {
                amount: 5,
                target: TargetRequirement::Creature,
            }]
    {
        return Err(
            "Fiery Conclusion typed definition differs from the feature contract".to_owned(),
        );
    }
    let cost = candidate_fiery_conclusion::additional_spell_cost_binding();
    if cost.card_definition != CARD_ID
        || cost.cost != AdditionalSpellCost::SacrificeControlledCreature
    {
        return Err(
            "Fiery Conclusion additional-cost binding differs from the feature contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Fiery Conclusion source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Fiery Conclusion source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 4 {
        return Err("Fiery Conclusion source has unsupported module items".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "AdditionalSpellCost"],
        vec!["cardbench_magic_engine", "AdditionalSpellCostBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|p| p.into_iter().map(str::to_owned).collect())
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
                    return Err("Fiery Conclusion source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) => {
                if functions
                    .insert(function.sig.ident.to_string(), function)
                    .is_some()
                {
                    return Err("Fiery Conclusion source has duplicate functions".to_owned());
                }
            }
            _ => {
                return Err("Fiery Conclusion source contains executable top-level code".to_owned())
            }
        }
    }
    if imports != expected {
        return Err("Fiery Conclusion imports differ from the source contract".to_owned());
    }
    if functions.len() != 2 {
        return Err("Fiery Conclusion source needs definition() and its binding helper".to_owned());
    }
    let function = functions
        .get("definition")
        .ok_or("Fiery Conclusion definition() is missing")?;
    let literal = direct_struct_function(function, "CardDefinition")?;
    let fields = literal_fields(literal)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Fiery Conclusion")
        || !check_string(fields["set_code"], "RAV")
        || !check_red_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Red"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Instant"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "additional-sacrifice-controlled-creature-cost",
                "targeted-creature-damage",
            ],
        )
        || !check_none(fields["power"])
        || !check_none(fields["toughness"])
        || !check_empty_vec(fields["keywords"])
        || !check_targeted_damage(fields["effects"], 5)
    {
        return Err(
            "Fiery Conclusion definition is outside its literal source contract".to_owned(),
        );
    }
    let binding = functions
        .get("additional_spell_cost_binding")
        .ok_or("Fiery Conclusion additional-cost binding is missing")?;
    let literal = direct_struct_function(binding, "AdditionalSpellCostBinding")?;
    let fields = literal_fields(literal)?;
    if fields.len() != 2
        || !check_string(fields["card_definition"], CARD_ID)
        || !check_sacrifice_cost(fields["cost"])
    {
        return Err("Fiery Conclusion binding is outside its literal source contract".to_owned());
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
        return Err("Fiery Conclusion definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Fiery Conclusion definition() must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Fiery Conclusion has the wrong literal type".to_owned());
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
fn check_red_cost(expr: &Expr) -> bool {
    let Some(args) = call_expr(expr, "ManaCost", "with_colors") else {
        return false;
    };
    args.len() == 2
        && check_int(args[0], 1)
        && matches!(args[1], Expr::Array(a) if a.elems.len() == 1
        && matches!(&a.elems[0], Expr::Path(p) if p.qself.is_none() && path_is(&p.path, &["Color", "Red"])))
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
fn check_empty_vec(expr: &Expr) -> bool {
    vec_items(expr).is_some_and(|items| items.is_empty())
}
fn check_targeted_damage(expr: &Expr, amount: i64) -> bool {
    let Some(items) = vec_items(expr) else {
        return false;
    };
    if items.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &items[0] else {
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
            syn::Member::Unnamed(_) => None,
            syn::Member::Named(_) => None,
        })
        .collect();
    effect.attrs.is_empty()
        && fields.len() == 2
        && fields.get("amount").is_some_and(|field| check_int(field, amount))
        && fields.get("target").is_some_and(|field| {
            matches!(field, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["TargetRequirement", "Creature"]))
        })
}

fn check_sacrifice_cost(expr: &Expr) -> bool {
    matches!(expr, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, &["AdditionalSpellCost", "SacrificeControlledCreature"]))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    DamageFour,
    CostRemoved,
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
    sacrifice: Option<ObjectId>,
    life: [i64; 2],
    spell_zone: Option<Zone>,
    sacrifice_zone: Option<Zone>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_FIERY_CONCLUSION_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_FIERY_CONCLUSION_SEED must be an unsigned integer".to_owned()
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
        "no Fiery Conclusion combat-removal trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_fiery_conclusion::definition();
    match mutation {
        Mutation::None => {}
        Mutation::DamageFour => {
            definition.effects = vec![Effect::DealDamage {
                amount: 4,
                target: TargetRequirement::Creature,
            }]
        }
        Mutation::CostRemoved | Mutation::GenericCost => {}
    }
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
                card: ELF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WOLFWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
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
                card: WOLFWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
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
        return Err("Fiery Conclusion fixtures must be shuffled 60-card decks".to_owned());
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
        Box::new(FieryPolicy::new(
            PlayerId(0),
            mutation == Mutation::GenericCost,
            mutation == Mutation::CostRemoved,
        )),
        Box::new(OpponentPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Fiery Conclusion match hit a bounded game limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Fiery Conclusion policy seat missing")?;
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
    }
    let spell = game
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
        .ok_or("Fiery Conclusion was not cast in the full game")?;
    let target = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::DamageDealtToPermanent {
                source, permanent, ..
            } if *source == spell => Some(*permanent),
            _ => None,
        })
        .ok_or("Fiery Conclusion did not damage its creature target")?;
    let sacrifice = game.event_log.iter().find_map(|event| match event {
        GameEvent::SacrificedAsAdditionalSpellCost {
            player: PlayerId(0),
            card,
            permanent,
        } if *card == spell => Some(*permanent),
        _ => None,
    });
    let winner = game.winner().map(|p| p.0);
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
        sacrifice,
        life: [game.players[0].life, game.players[1].life],
        spell_zone: game.zone_of(spell),
        sacrifice_zone: sacrifice.map(|permanent| game.zone_of(permanent)).flatten(),
    })
}

fn new_rav_game(definition: CardDefinition, mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Fiery Conclusion missing from RAV definition registry")?;
    *registered = definition;
    let mut additional_spell_costs = rav_additional_spell_cost_bindings();
    additional_spell_costs.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::CostRemoved {
        additional_spell_costs.push(candidate_fiery_conclusion::additional_spell_cost_binding());
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        additional_spell_costs,
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

fn green_white_red_total(view: &GameView) -> usize {
    [Color::Green, Color::White, Color::Red]
        .into_iter()
        .map(|color| usize::from(view.mana_pool.amount(color)))
        .sum()
}

fn land_color_count(view: &GameView, color: Color) -> usize {
    view.own_battlefield
        .iter()
        .filter(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
        })
        .count()
}

fn select_land_color(view: &GameView, need_red: bool) -> Color {
    if land_color_count(view, Color::Green) == 0 {
        return Color::Green;
    }
    if land_color_count(view, Color::White) == 0 {
        return Color::White;
    }
    if need_red && land_color_count(view, Color::Red) == 0 {
        return Color::Red;
    }
    let colors: &[Color] = if need_red {
        &[Color::Green, Color::White, Color::Red]
    } else {
        &[Color::Green, Color::White]
    };
    colors
        .iter()
        .copied()
        .min_by_key(|color| land_color_count(view, *color))
        .unwrap_or(Color::Green)
}

fn play_land(view: &GameView, need_red: bool) -> Option<PolicyAction> {
    if view.lands_played != 0 {
        return None;
    }
    let color = select_land_color(view, need_red);
    view.hand
        .iter()
        .find(|card| card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color))
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

struct FieryPolicy {
    player: PlayerId,
    elf_cast: bool,
    wolf_cast: bool,
    fiery_cast: bool,
    generic_cost_mutation: bool,
    sacrifice_cost_removed: bool,
}
impl FieryPolicy {
    fn new(player: PlayerId, generic_cost_mutation: bool, sacrifice_cost_removed: bool) -> Self {
        Self {
            player,
            elf_cast: false,
            wolf_cast: false,
            fiery_cast: false,
            generic_cost_mutation,
            sacrifice_cost_removed,
        }
    }
    fn cast_elf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.elf_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let elf = view.hand.iter().find(|card| card.definition == Some(ELF))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return activate_land_color(view, Color::Green);
        }
        self.elf_cast = true;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: elf.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
    fn cast_wolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        if !self.elf_cast
            || self.wolf_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let wolf = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WOLFWOLF))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return activate_land_color(view, Color::Green);
        }
        if view.mana_pool.amount(Color::White) == 0 {
            return activate_land_color(view, Color::White);
        }
        self.wolf_cast = true;
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: wolf.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
    fn cast_fiery_during_attack(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.fiery_cast
            || self.player != PlayerId(0)
            || view.active_player == self.player
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
            .find(|attacker| attacker.definition == Some(WOLFWOLF))?;
        let fiery = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let sacrifice = if self.sacrifice_cost_removed {
            None
        } else {
            Some(
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(ELF))?
                    .id,
            )
        };
        let generic_colors = if self.generic_cost_mutation {
            if view.mana_pool.amount(Color::Green) == 0 {
                return activate_land_color(view, Color::Green);
            }
            if view.mana_pool.amount(Color::White) == 0 {
                return activate_land_color(view, Color::White);
            }
            vec![Color::Green, Color::White]
        } else {
            if view.mana_pool.amount(Color::Red) == 0 {
                return activate_land_color(view, Color::Red);
            }
            if green_white_red_total(view) < 2 {
                return activate_any_land(view);
            }
            let generic_color = if view.mana_pool.amount(Color::Green) > 0 {
                Color::Green
            } else if view.mana_pool.amount(Color::White) > 0 {
                Color::White
            } else if view.mana_pool.amount(Color::Red) > 1 {
                Color::Red
            } else {
                return activate_any_land(view);
            };
            vec![generic_color]
        };
        self.fiery_cast = true;
        let mut targets = vec![Target::Permanent(attacker.id)];
        if let Some(sacrifice) = sacrifice {
            targets.push(Target::SacrificePermanent(sacrifice));
        }
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: fiery.id,
                targets,
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: generic_colors,
                hybrid: vec![],
            },
        })
    }
}
impl CodePolicy for FieryPolicy {
    fn id(&self) -> &'static str {
        "rav.fiery-conclusion-combat-removal.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(1) || self.fiery_cast {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WOLFWOLF) && card.can_attack)
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
            let assignment = view
                .combat_attackers
                .iter()
                .find(|attacker| attacker.definition == Some(ELF))
                .and_then(|attacker| {
                    view.own_battlefield
                        .iter()
                        .find(|blocker| blocker.definition == Some(WOLFWOLF) && blocker.can_block)
                        .map(|blocker| CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        })
                });
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.cast_fiery_during_attack(view) {
            return action;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = play_land(view, true) {
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

struct OpponentPolicy {
    player: PlayerId,
    elf_cast: bool,
    wolf_cast: bool,
}
impl OpponentPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_cast: false,
            wolf_cast: false,
        }
    }
}
impl CodePolicy for OpponentPolicy {
    fn id(&self) -> &'static str {
        "rav.fiery-conclusion-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let has_attacking_watchwolf = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WOLFWOLF) && card.can_attack);
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.can_attack
                        && (card.definition == Some(WOLFWOLF)
                            || (has_attacking_watchwolf && card.definition == Some(ELF)))
                })
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
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
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = play_land(view, false) {
            return action;
        }
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
        if !self.wolf_cast && self.elf_cast {
            if let Some(wolf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WOLFWOLF))
            {
                if view.mana_pool.amount(Color::Green) == 0 {
                    return activate_land_color(view, Color::Green)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                if view.mana_pool.amount(Color::White) == 0 {
                    return activate_land_color(view, Color::White)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                self.wolf_cast = true;
                return PolicyAction::Cast(cardbench_magic_engine::CastRequest {
                    card: wolf.id,
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
        return Err("Fiery Conclusion match did not end in a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Fiery Conclusion match contains rejected policy moves".to_owned());
    }
    if captured.life[0] <= 0 || captured.life[1] > 0 {
        return Err(format!(
            "Fiery Conclusion full game did not end with seat zero alive and seat one defeated: {:?}",
            captured.life
        ));
    }
    let sacrifice = captured
        .sacrifice
        .ok_or("Fiery Conclusion additional-cost sacrifice receipt is missing")?;
    if captured.spell_zone != Some(Zone::Graveyard)
        || captured.sacrifice_zone != Some(Zone::Graveyard)
    {
        return Err(format!(
            "Fiery Conclusion source and sacrificed creature did not reach the graveyard: spell={:?}, sacrifice={:?}",
            captured.spell_zone, captured.sacrifice_zone
        ));
    }
    let attack = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&captured.target))
    })
    .ok_or("opponent Watchwolf attack declaration is missing")?;
    let payment = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.spell)).ok_or("Fiery Conclusion exact spell payment receipt is missing")?;
    let sacrifice_receipt = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::SacrificedAsAdditionalSpellCost { player: PlayerId(0), card, permanent } if *card == captured.spell && *permanent == sacrifice)
    })
    .ok_or("Fiery Conclusion was not paid for by sacrificing the selected creature")?;
    let sacrifice_move = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == sacrifice)
    })
    .ok_or("the additional-cost creature did not move to its graveyard")?;
    let cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.spell)).ok_or("Fiery Conclusion cast event is missing")?;
    let damage = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 5 } if *source == captured.spell && *permanent == captured.target)
    })
    .ok_or("Fiery Conclusion did not deal five damage to its attacking creature target")?;
    let resolved = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.spell),
    )
    .ok_or("Fiery Conclusion did not resolve")?;
    let fiery_graveyard = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.spell)).ok_or("Fiery Conclusion did not move to its owner's graveyard")?;
    let target_sba = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::StateBasedAction { card, .. } if *card == captured.target)
    })
    .ok_or("lethal damage did not trigger the target creature's state-based action")?;
    let target_graveyard = unique_event_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.target)
    })
    .ok_or("the damaged attacking creature did not move to its graveyard")?;
    let blockers = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::BlockersDeclared {
                player: PlayerId(0),
                ..
            }
        )
    })
    .ok_or("defending player did not reach the blockers declaration")?;
    let end = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Fiery Conclusion full game lacks terminal winner event")?;
    let mana_colors: Vec<_> = match &captured.events[payment] {
        GameEvent::SpellManaPaid { colors, .. } => {
            colors.iter().map(|color| format!("{color:?}")).collect()
        }
        _ => unreachable!(),
    };
    if mana_colors.len() != 2 || mana_colors.iter().filter(|color| *color == "Red").count() != 1 {
        return Err(format!(
            "Fiery Conclusion paid colors do not prove {{1}}{{R}}: {mana_colors:?}"
        ));
    }
    if !(attack < sacrifice_receipt
            && sacrifice_receipt + 1 == sacrifice_move
            && sacrifice_move < payment
            && sacrifice_move < cast
            && cast < damage
            && damage < resolved
            && resolved < fiery_graveyard
            && fiery_graveyard < target_sba
            && target_sba < target_graveyard
            && target_graveyard < blockers
            && blockers < end)
        || captured
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), .. } if *source == captured.target))
    {
        return Err(format!(
            "Fiery Conclusion combat response, cost, removal, or terminal order differs: attack={attack}, payment={payment}, sacrifice={sacrifice_receipt}/{sacrifice_move}, cast={cast}, damage={damage}, resolve={resolved}, spell_move={fiery_graveyard}, target_sba={target_sba}, target_move={target_graveyard}, blockers={blockers}, end={end}, life={:?}",
            captured.life
        ));
    }
    let events: Vec<_> = captured
        .events
        .iter()
        .filter(|event| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card } => *card == captured.spell,
            GameEvent::AttackersDeclared {
                player: PlayerId(1),
                attackers,
            } => attackers.contains(&captured.target),
            GameEvent::SacrificedAsAdditionalSpellCost {
                card, permanent, ..
            } => *card == captured.spell && *permanent == sacrifice,
            GameEvent::DamageDealtToPermanent { source, .. } => *source == captured.spell,
            GameEvent::StateBasedAction { card, .. } => *card == captured.target,
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => [sacrifice, captured.spell, captured.target].contains(card),
            GameEvent::BlockersDeclared {
                player: PlayerId(0),
                ..
            } => true,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .cloned()
        .collect();
    let projected = project_events(&events);
    Ok(json!({
        "trace_id": "rav-fiery-conclusion-kills-attacker-for-additional-cost",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": events.len(),
        "spell_object": captured.spell.0,
        "attacking_target": captured.target.0,
        "sacrificed_creature": sacrifice.0,
        "mana_cost": "{1}{R}",
        "mana_paid": mana_colors,
        "effect": {"damage": 5, "target_zone_after_resolution": "Graveyard", "target_removed_before_combat_damage": true},
        "additional_cost": {"kind": "sacrifice-controlled-creature", "selected_zone_after_payment": "Graveyard"},
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
        Err(_) => false,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 122;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" }, "credited": false})
}
