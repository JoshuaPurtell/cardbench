//! Full-game RAV Rain of Embers authority for simultaneous global damage.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
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
const CARD_ID: &str = "RAV-RAIN-OF-EMBERS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/rain_of_embers.rs";
const ELF: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WOLFWOLF: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/rain_of_embers.rs"]
mod candidate_rain_of_embers;

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
            eprintln!("Rain of Embers PvP authority failed: {error}");
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
        return Err("Rain of Embers shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "two_damage_mutation_detected_in_game": mutation_detected(seed, Mutation::DamageTwo),
        "creatures_only_mutation_detected_in_game": mutation_detected(seed, Mutation::CreaturesOnly),
        "red_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "two_damage_mutation_detected_in_game",
        "creatures_only_mutation_detected_in_game",
        "red_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Rain of Embers gameplay mutation escaped the oracle: {key}"
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
        "card": {"collector_number": 138, "definition_id": CARD_ID, "name": "Rain of Embers"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["120.3-120.6 Damage results and marked damage", "601.2 Casting spells and paying costs", "608.2-608.3 Resolving spells", "704.5g Lethal-damage state-based action"],
            "source_contract": "RAV Rain of Embers identity, red sorcery cost, and one simultaneous one-damage batch to every creature and player",
            "authority_cutoff": "2026-09-24"
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
        "replay_deterministic": true,
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
    let definition = candidate_rain_of_embers::definition();
    if definition.id != CARD_ID
        || definition.name != "Rain of Embers"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != [Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }]
    {
        return Err("Rain of Embers typed definition differs from the feature contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Rain of Embers source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Rain of Embers source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Rain of Embers source has unsupported module items".to_owned());
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
                    return Err("Rain of Embers source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function) if definition.is_none() => definition = Some(function),
            _ => return Err("Rain of Embers source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected {
        return Err("Rain of Embers imports differ from the source contract".to_owned());
    }
    let function = definition.ok_or("Rain of Embers definition() is missing")?;
    let literal = direct_struct_function(function, "CardDefinition")?;
    let fields = literal_fields(literal)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Rain of Embers")
        || !check_string(fields["set_code"], "RAV")
        || !check_red_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Red"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Sorcery"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &["full-rules-fidelity", "global-creature-and-player-damage"],
        )
        || !check_none(fields["power"])
        || !check_none(fields["toughness"])
        || !check_empty_vec(fields["keywords"])
        || !check_global_damage(fields["effects"], 1)
    {
        return Err("Rain of Embers definition is outside its literal source contract".to_owned());
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
        return Err("Rain of Embers definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Rain of Embers definition() must return one literal".to_owned());
    };
    if !path_is(&value.path, &[return_type]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Rain of Embers has the wrong literal type".to_owned());
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
fn check_global_damage(expr: &Expr, amount: i64) -> bool {
    let Some(items) = vec_items(expr) else {
        return false;
    };
    if items.len() != 1 {
        return false;
    }
    let Expr::Struct(effect) = &items[0] else {
        return false;
    };
    path_is(
        &effect.path,
        &["Effect", "DealDamageToEachCreatureAndPlayer"],
    ) && effect.rest.is_none()
        && effect.fields.len() == 1
        && matches!(&effect.fields[0].member, syn::Member::Named(name) if name == "amount")
        && check_int(&effect.fields[0].expr, amount)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    DamageTwo,
    CreaturesOnly,
    GenericCost,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    rain: ObjectId,
    elf0: ObjectId,
    elf1: ObjectId,
    wolf0: ObjectId,
    wolf1: ObjectId,
    life: [i64; 2],
    wolf_damage: [i32; 2],
    zones: [Option<Zone>; 4],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_RAIN_OF_EMBERS_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_RAIN_OF_EMBERS_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut failures = Vec::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None) {
            Ok(captured) => match validate_trace(&captured, seed) {
                Ok(_) => return Ok((seed, captured)),
                Err(error) => failures.push(format!("seed {seed}: {error}")),
            },
            Err(error) => failures.push(format!("seed {seed}: {error}")),
        }
    }
    Err(format!(
        "no terminal Rain of Embers global-damage trace found for seeds 0..={MAX_TRACE_SEED}; first failures: {}",
        failures.iter().take(5).cloned().collect::<Vec<_>>().join(" | ")
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_rain_of_embers::definition();
    match mutation {
        Mutation::None => {}
        Mutation::DamageTwo => {
            definition.effects = vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 2 }]
        }
        Mutation::CreaturesOnly => {
            definition.effects = vec![Effect::DealDamageToEachNonFlyingCreature { amount: 1 }]
        }
        Mutation::GenericCost => definition.mana_cost = ManaCost::new(2),
    }
    let mut game = new_rav_game(definition)?;
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
        return Err("Rain of Embers fixtures must be shuffled 60-card decks".to_owned());
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
        Box::new(RainPolicy::new(
            PlayerId(0),
            mutation == Mutation::GenericCost,
        )),
        Box::new(SupportPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut snapshot: Option<(
        ObjectId,
        ObjectId,
        ObjectId,
        ObjectId,
        ObjectId,
        [i64; 2],
        [i32; 2],
        [Option<Zone>; 4],
    )> = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Rain of Embers match hit a bounded game limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Rain of Embers policy seat missing")?;
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
        if snapshot.is_none() {
            let rain = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast { player: PlayerId(0), card } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => {
                    game.event_log.iter().any(|later| matches!(later, GameEvent::SpellResolved { card: resolved } if resolved == card)).then_some(*card)
                }
                _ => None,
            });
            if let Some(rain) = rain {
                let elf0 = find_card_in_zone(&game, PlayerId(0), ELF, Zone::Graveyard)
                    .ok_or("player zero's Elves of Deep Shadow did not die to Rain of Embers")?;
                let elf1 = find_card_in_zone(&game, PlayerId(1), ELF, Zone::Graveyard)
                    .ok_or("player one's Elves of Deep Shadow did not die to Rain of Embers")?;
                let wolf0 = find_card_in_zone(&game, PlayerId(0), WOLFWOLF, Zone::Battlefield)
                    .ok_or("player zero's Watchwolf did not survive Rain of Embers")?;
                let wolf1 = find_card_in_zone(&game, PlayerId(1), WOLFWOLF, Zone::Battlefield)
                    .ok_or("player one's Watchwolf did not survive Rain of Embers")?;
                let p0_damage = game.object(wolf0).map_err(|e| e.to_string())?.damage;
                let p1_damage = game.object(wolf1).map_err(|e| e.to_string())?.damage;
                snapshot = Some((
                    rain,
                    elf0,
                    elf1,
                    wolf0,
                    wolf1,
                    [game.players[0].life, game.players[1].life],
                    [p0_damage, p1_damage],
                    [
                        game.zone_of(elf0),
                        game.zone_of(elf1),
                        game.zone_of(wolf0),
                        game.zone_of(wolf1),
                    ],
                ));
            }
        }
    }
    let (rain, elf0, elf1, wolf0, wolf1, life, wolf_damage, zones) = snapshot.ok_or(
        "Rain of Embers did not resolve with creatures from both players on the battlefield",
    )?;
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
        rain,
        elf0,
        elf1,
        wolf0,
        wolf1,
        life,
        wolf_damage,
        zones,
    })
}

fn find_card_in_zone(
    game: &Game,
    player: PlayerId,
    definition: &str,
    zone: Zone,
) -> Option<ObjectId> {
    let cards = match zone {
        Zone::Battlefield => &game.players[player.0].battlefield,
        Zone::Graveyard => &game.players[player.0].graveyard,
        _ => return None,
    };
    cards.iter().copied().find(|card| {
        game.card_definition(*card)
            .is_ok_and(|d| d.id == definition)
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Rain of Embers missing from RAV definition registry")?;
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

struct RainPolicy {
    player: PlayerId,
    elf_cast: bool,
    wolf_cast: bool,
    rain_cast: bool,
    generic_cost_mutation: bool,
}
impl RainPolicy {
    fn new(player: PlayerId, generic_cost_mutation: bool) -> Self {
        Self {
            player,
            elf_cast: false,
            wolf_cast: false,
            rain_cast: false,
            generic_cost_mutation,
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
    fn cast_rain(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.rain_cast
            || !self.wolf_cast
            || !self.elf_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        if !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(ELF))
            || !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WOLFWOLF))
            || !view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(ELF))
            || !view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(WOLFWOLF))
        {
            return None;
        }
        let rain = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
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
        self.rain_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: rain.id,
                targets: vec![],
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
impl CodePolicy for RainPolicy {
    fn id(&self) -> &'static str {
        "rav.rain-of-embers-global-damage.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.rain_cast {
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
        if let Some(action) = play_land(view, true) {
            return action;
        }
        if let Some(action) = self.cast_elf(view) {
            return action;
        }
        if let Some(action) = self.cast_wolf(view) {
            return action;
        }
        if let Some(action) = self.cast_rain(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct SupportPolicy {
    player: PlayerId,
    elf_cast: bool,
    wolf_cast: bool,
}
impl SupportPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_cast: false,
            wolf_cast: false,
        }
    }
}
impl CodePolicy for SupportPolicy {
    fn id(&self) -> &'static str {
        "rav.rain-of-embers-opponent-board.v1"
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
        return Err("Rain of Embers match did not end in a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Rain of Embers match contains rejected policy moves".to_owned());
    }
    if captured.life != [19, 19]
        || captured.wolf_damage != [1, 1]
        || captured.zones
            != [
                Some(Zone::Graveyard),
                Some(Zone::Graveyard),
                Some(Zone::Battlefield),
                Some(Zone::Battlefield),
            ]
    {
        return Err(format!(
            "Rain of Embers post-batch battlefield/life state differs: life={:?}, wolf_damage={:?}, zones={:?}",
            captured.life, captured.wolf_damage, captured.zones
        ));
    }
    let payment = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.rain)).ok_or_else(|| format!("Rain of Embers exact spell payment receipt is missing; rain={:?}; related={:?}", captured.rain, captured.events.iter().filter(|event| matches!(event, GameEvent::SpellManaPaid { .. } | GameEvent::SpellCast { .. } | GameEvent::SpellResolved { .. })).collect::<Vec<_>>()))?;
    let cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.rain)).ok_or("Rain of Embers cast event is missing")?;
    let resolved = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.rain),
    )
    .ok_or("Rain of Embers did not resolve")?;
    let rain_graveyard = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.rain)).ok_or("Rain of Embers did not move to its owner's graveyard")?;
    let end = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Rain of Embers full game lacks terminal winner event")?;
    let mana_colors: Vec<_> = match &captured.events[payment] {
        GameEvent::SpellManaPaid { colors, .. } => {
            colors.iter().map(|color| format!("{color:?}")).collect()
        }
        _ => unreachable!(),
    };
    if mana_colors.len() != 2 || mana_colors.iter().filter(|color| *color == "Red").count() != 1 {
        return Err(format!(
            "Rain of Embers paid colors do not prove {{1}}{{R}}: {mana_colors:?}"
        ));
    }
    let creature_hits: Vec<_> = captured
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount: 1,
            } if *source == captured.rain => Some((index, *permanent)),
            _ => None,
        })
        .collect();
    let expected_creatures =
        BTreeSet::from([captured.elf0, captured.elf1, captured.wolf0, captured.wolf1]);
    if creature_hits.len() != 4
        || creature_hits
            .iter()
            .map(|(_, card)| *card)
            .collect::<BTreeSet<_>>()
            != expected_creatures
    {
        return Err(format!(
            "Rain of Embers did not deal exactly one damage to every current creature: {creature_hits:?}"
        ));
    }
    let player_hits: Vec<_> = captured
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount: 1,
            } if *source == captured.rain => Some((index, *player)),
            _ => None,
        })
        .collect();
    if player_hits.len() != 2
        || player_hits
            .iter()
            .map(|(_, player)| *player)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([PlayerId(0), PlayerId(1)])
    {
        return Err(format!(
            "Rain of Embers did not damage both live players once: {player_hits:?}"
        ));
    }
    let death0 = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.elf0)).ok_or("player zero's damaged Elf did not die after the batch")?;
    let death1 = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.elf1)).ok_or("player one's damaged Elf did not die after the batch")?;
    let all_damage_indices = creature_hits
        .iter()
        .map(|(index, _)| *index)
        .chain(player_hits.iter().map(|(index, _)| *index))
        .collect::<Vec<_>>();
    let last_damage = *all_damage_indices
        .iter()
        .max()
        .ok_or("global damage receipts are missing")?;
    let first_sba_death = death0.min(death1);
    let moved_spell_after_resolve = resolved < rain_graveyard;
    if !(payment < cast
        && cast < last_damage
        && last_damage < first_sba_death
        && resolved < rain_graveyard
        && rain_graveyard < end
        && moved_spell_after_resolve)
    {
        return Err(format!(
            "Rain of Embers payment, damage batch, state-based action, resolution, or terminal ordering differs: payment={payment}, cast={cast}, last_damage={last_damage}, deaths={death0}/{death1}, resolve={resolved}, spell_move={rain_graveyard}, end={end}"
        ));
    }
    let events: Vec<_> = captured
        .events
        .iter()
        .filter(|event| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card } => *card == captured.rain,
            GameEvent::DamageDealtToPermanent { source, .. }
            | GameEvent::DamageDealtToPlayer { source, .. } => *source == captured.rain,
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => [captured.elf0, captured.elf1, captured.rain].contains(card),
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .cloned()
        .collect();
    let projected = project_events(&events);
    Ok(json!({
        "trace_id": "rav-rain-of-embers-global-damage-batch",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": events.len(),
        "rain_object": captured.rain.0,
        "mana_cost": "{1}{R}",
        "mana_paid": mana_colors,
        "players": [{"seat": 0, "life_after_batch": captured.life[0]}, {"seat": 1, "life_after_batch": captured.life[1]}],
        "creatures": [
            {"seat": 0, "elf": captured.elf0.0, "elf_zone_after_batch": "Graveyard", "watchwolf": captured.wolf0.0, "watchwolf_damage": captured.wolf_damage[0], "watchwolf_zone_after_batch": "Battlefield"},
            {"seat": 1, "elf": captured.elf1.0, "elf_zone_after_batch": "Graveyard", "watchwolf": captured.wolf1.0, "watchwolf_damage": captured.wolf_damage[1], "watchwolf_zone_after_batch": "Battlefield"}
        ],
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
    let passed = card.collector_number == 138;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" }, "credited": false})
}
