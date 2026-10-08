//! Full-game diagnostic authority for RAV Vedalken Entrancer.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VEDALKEN-ENTRANCER";
const ABILITY_ID: &str = "tap-blue-mill-two";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ISLAND_ID: &str = "RAV-ISLAND";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_entrancer.rs";
const MAX_TRACE_SEED: u64 = 32;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/vedalken_entrancer.rs"]
mod candidate_vedalken_entrancer;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Vedalken Entrancer result: {error}");
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
            eprintln!("Vedalken Entrancer PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MillOne,
    NoTapCost,
    GenericCost,
    NoPlayerTarget,
    NoDefender,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    entrancer: Option<ObjectId>,
    entrancer_spell_payment: Vec<Color>,
    entrancer_resolved: bool,
    entrancer_entered: bool,
    entrancer_power: Option<i32>,
    entrancer_toughness: Option<i32>,
    has_defender: bool,
    ability_source: Option<ObjectId>,
    ability_target: Option<Target>,
    ability_cost: Option<ManaCost>,
    source_tapped_after_activation: bool,
    source_zone_after_resolution: Option<Zone>,
    activation_active_player: Option<PlayerId>,
    library_before: Option<usize>,
    library_after: Option<usize>,
    expected_milled: Vec<ObjectId>,
    milled: Vec<ObjectId>,
    milled_owners: Vec<PlayerId>,
    watchwolf: Option<ObjectId>,
    attack_turns: Vec<u32>,
    damage_to_opponent: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Vedalken Entrancer full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "mill_amount_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::MillOne)?,
        "tap_cost_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "blue_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost)?,
        "player_target_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoPlayerTarget)?,
        "defender_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoDefender)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "mill_amount_changed_mutation_detected_in_game",
        "tap_cost_removed_mutation_detected_in_game",
        "blue_cost_changed_mutation_detected_in_game",
        "player_target_removed_mutation_detected_in_game",
        "defender_removed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Vedalken Entrancer gameplay mutant escaped: {key}"));
        }
    }
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 74, "printings": 1, "definition_id": CARD_ID, "name": "Vedalken Entrancer"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "302.6: summoning sickness and tap-symbol costs",
                "602.2: activating an ability and paying costs",
                "608.2: resolving an activated ability",
                "701.13: milling cards",
                "508.1 and 510.1: declaring attackers and combat damage"
            ],
            "source_contract": "RAV Vedalken Entrancer #74 is a blue 1/4 Creature with Defender and {U}, {T}: target player mills two cards.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_vedalken_entrancer::definition();
    if mutation == Mutation::NoDefender {
        definition.keywords.clear();
    }
    definition
}

fn candidate_ability(mutation: Mutation) -> ActivatedAbility {
    let mut ability = candidate_vedalken_entrancer::activated_ability();
    match mutation {
        Mutation::None | Mutation::NoDefender => {}
        Mutation::MillOne => ability.effects = vec![Effect::MillTargetPlayer { count: 1 }],
        Mutation::NoTapCost => ability.tap_cost = false,
        Mutation::GenericCost => ability.mana_cost = ManaCost::new(1),
        Mutation::NoPlayerTarget => ability.targets.clear(),
    }
    ability
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_vedalken_entrancer::definition();
    let ability = candidate_vedalken_entrancer::activated_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vedalken Entrancer is missing from the RAV definition registry")?;
    let bindings = rav_activated_ability_bindings();
    let candidates: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if registered != definition
        || candidates.len() != 1
        || candidates[0].ability != ability
        || definition != expected_definition()
        || ability != expected_ability()
    {
        return Err("Vedalken Entrancer candidate definition or its single activated ability differs from the RAV source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Vedalken Entrancer source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Vedalken Entrancer source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Vedalken Entrancer module must contain only private imports and two direct-literal functions".to_owned());
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
                if !safe_use_tree(&item.tree) {
                    return Err("Vedalken Entrancer contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let unsafe_fields: Vec<_> = function
                    .block
                    .stmts
                    .first()
                    .and_then(|statement| match statement {
                        Stmt::Expr(Expr::Struct(literal), None) => Some(literal),
                        _ => None,
                    })
                    .map(|literal| {
                        literal
                            .fields
                            .iter()
                            .filter(|field| !safe_expression(&field.expr))
                            .map(|field| match &field.member {
                                syn::Member::Named(name) => name.to_string(),
                                syn::Member::Unnamed(index) => index.index.to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !functions.insert(name.clone())
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                    })
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(format!(
                        "Vedalken Entrancer function `{name}` is not a direct declarative literal (crate_vis={}, statements={}, unsafe_fields={unsafe_fields:?})",
                        matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate")),
                        function.block.stmts.len()
                    ));
                }
            }
            _ => {
                return Err(
                    "Vedalken Entrancer source contains an executable or unsupported item"
                        .to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("Vedalken Entrancer source imports or function identity differs from the source contract".to_owned());
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet"
                | "ActivatedAbility"
                | "CardDefinition"
                | "CardType"
                | "Color"
                | "Effect"
                | "Keyword"
                | "ManaCost"
                | "TargetRequirement"
        ),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["None"])
                || path_is(&segments, &["Color", "Blue"])
                || path_is(&segments, &["CardType", "Creature"])
                || path_is(&segments, &["Keyword", "Defender"])
                || path_is(&segments, &["TargetRequirement", "Player"])
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            let valid = path_is(&segments, &["ManaCost", "with_colors"])
                || path_is(&segments, &["ManaCost", "new"])
                || path_is(&segments, &["BTreeSet", "from"])
                || path_is(&segments, &["BTreeSet", "new"])
                || path_is(&segments, &["Some"]);
            valid && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            let segments: Vec<_> = literal
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (path_is(&segments, &["CardDefinition"])
                || path_is(&segments, &["ActivatedAbility"])
                || path_is(&segments, &["Effect", "MillTargetPlayer"]))
                && literal.rest.is_none()
                && literal
                    .fields
                    .iter()
                    .all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Vedalken Entrancer definition is missing from the RAV registry")? =
        candidate_definition(mutation);
    let mut abilities = rav_activated_ability_bindings();
    let matching: Vec<_> = abilities
        .iter_mut()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if matching.len() != 1 {
        return Err(format!(
            "expected one Entrancer activation binding, found {}",
            matching.len()
        ));
    }
    matching.into_iter().next().expect("one binding").ability = candidate_ability(mutation);
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_VEDALKEN_ENTRANCER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_VEDALKEN_ENTRANCER_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Vedalken Entrancer full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        (ISLAND_ID, 20),
        (FOREST_ID, 16),
        (PLAINS_ID, 16),
    ]);
    let opponent = deck(&[(ISLAND_ID, 60)]);
    if [&pilot, &opponent].iter().any(|list| {
        list.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Vedalken Entrancer fixtures must be two legal 60-card decks".to_owned());
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(EntrancerPolicy::new(PlayerId(0), mutation)),
        Box::new(PassPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        entrancer: None,
        entrancer_spell_payment: vec![],
        entrancer_resolved: false,
        entrancer_entered: false,
        entrancer_power: None,
        entrancer_toughness: None,
        has_defender: false,
        ability_source: None,
        ability_target: None,
        ability_cost: None,
        source_tapped_after_activation: false,
        source_zone_after_resolution: None,
        activation_active_player: None,
        library_before: None,
        library_after: None,
        expected_milled: vec![],
        milled: vec![],
        milled_owners: vec![],
        watchwolf: None,
        attack_turns: vec![],
        damage_to_opponent: 0,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted >= config.max_policy_moves {
            return Err(format!(
                "Vedalken Entrancer duel exceeded its action bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Vedalken Entrancer policy seat missing")?;
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
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            captured.entrancer = Some(request.card);
        }
        let activation_source_to_check = if let PolicyAction::ActivateAbility { activation } =
            &action
            && activation.ability_id == ABILITY_ID
        {
            captured.ability_source = Some(activation.source);
            captured.ability_target = activation.targets.first().cloned();
            captured.activation_active_player = Some(view.active_player);
            captured.library_before = Some(
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .library
                    .len(),
            );
            captured.expected_milled = game
                .player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .library
                .iter()
                .rev()
                .take(2)
                .copied()
                .collect();
            Some(activation.source)
        } else {
            None
        };
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Vedalken Entrancer move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        if let Some(source) = activation_source_to_check {
            captured.source_tapped_after_activation =
                game.object(source).is_ok_and(|object| object.tapped);
        }
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[cursor..], &mut captured)?;
    }
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.life = life;
    Ok(captured)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    let expected: HashSet<_> = captured.expected_milled.iter().copied().collect();
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == captured.entrancer => {
                captured.entrancer_spell_payment = colors.clone()
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.entrancer => {
                captured.entrancer_resolved = true;
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.entrancer => {
                captured.entrancer_entered = true;
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                captured.entrancer_power = characteristics.power;
                captured.entrancer_toughness = characteristics.toughness;
                captured.has_defender = characteristics.keywords.contains(&Keyword::Defender);
            }
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability,
                mana_cost,
            } if Some(*source) == captured.ability_source && *ability == ABILITY_ID => {
                captured.ability_cost = Some(mana_cost.clone());
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if expected.contains(card)
                && game
                    .object(*card)
                    .is_ok_and(|object| object.owner == PlayerId(1)) =>
            {
                if !captured.milled.contains(card) {
                    captured.milled.push(*card);
                    captured.milled_owners.push(PlayerId(1));
                }
            }
            GameEvent::AbilityResolved {
                source, ability, ..
            } if Some(*source) == captured.ability_source && *ability == ABILITY_ID => {
                captured.source_zone_after_resolution = game.zone_of(*source);
                captured.library_after = Some(
                    game.player(PlayerId(1))
                        .map_err(|error| error.to_string())?
                        .library
                        .len(),
                );
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
            {
                captured.watchwolf = Some(*card);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured
                .watchwolf
                .is_some_and(|wolf| attackers.contains(&wolf)) =>
            {
                captured.attack_turns.push(game.turn);
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if captured.watchwolf == Some(*source) => captured.damage_to_opponent += *amount,
            _ => {}
        }
    }
    Ok(())
}

struct EntrancerPolicy {
    player: PlayerId,
    mutation: Mutation,
    entrancer_cast: bool,
    watchwolf_cast: bool,
    activation_offered: bool,
}

impl EntrancerPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            entrancer_cast: false,
            watchwolf_cast: false,
            activation_offered: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::new();
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

    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                    .then(|| cost.colored.first().copied())
                    .flatten()
            })
            .unwrap_or(Color::Blue);
        let land = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&missing)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: missing,
        })
    }

    fn cast_card(
        &mut self,
        view: &GameView,
        id: &'static str,
        cost: &ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(id))?;
            if id == CARD_ID {
                self.entrancer_cast = true;
            } else if id == WATCHWOLF_ID {
                self.watchwolf_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        Self::make_mana(view, cost)
    }

    fn play_land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let have_blue = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Blue)
        });
        let have_green = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Green)
        });
        let have_white = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::White)
        });
        let preferred = if !have_blue {
            ISLAND_ID
        } else if !have_green {
            FOREST_ID
        } else if !have_white {
            PLAINS_ID
        } else {
            ISLAND_ID
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(ISLAND_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(FOREST_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(PLAINS_ID))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for EntrancerPolicy {
    fn id(&self) -> &'static str {
        "rav.vedalken-entrancer-target-player-mill-two.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
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
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && view.step.is_main() {
            if view.lands_played == 0
                && let Some(action) = Self::play_land(view)
            {
                return action;
            }
            if !self.entrancer_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                let cost = candidate_definition(self.mutation).mana_cost;
                if let Some(action) = self.cast_card(view, CARD_ID, &cost, vec![]) {
                    return action;
                }
            }
            if self.entrancer_cast
                && !self.watchwolf_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(action) = self.cast_card(view, WATCHWOLF_ID, &cost, vec![]) {
                    return action;
                }
            }
        }
        let live_source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick);
        if !self.activation_offered
            && view.active_player != self.player
            && let Some(source) = live_source
        {
            if view.mana_pool.amount(Color::Blue) == 0 {
                if let Some(action) =
                    Self::make_mana(view, &ManaCost::with_colors(0, [Color::Blue]))
                {
                    return action;
                }
            } else {
                self.activation_offered = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Player(PlayerId(1))],
                    },
                };
            }
        }
        PolicyAction::PassPriority
    }
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
        "rav.vedalken-entrancer-passive-opponent.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.player == self.player
            && view.step == Step::DeclareBlockers
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

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected != 0 {
        return Err("Vedalken Entrancer duel did not reach a clean P0 terminal win".to_owned());
    }
    let source = capture.entrancer.ok_or("Vedalken Entrancer was not cast")?;
    let ability_source = capture
        .ability_source
        .ok_or("Entrancer ability was not activated")?;
    if source != ability_source
        || !capture.entrancer_resolved
        || !capture.entrancer_entered
        || capture.entrancer_spell_payment.len() != 4
        || !capture.entrancer_spell_payment.contains(&Color::Blue)
        || capture.entrancer_power != Some(1)
        || capture.entrancer_toughness != Some(4)
        || !capture.has_defender
    {
        return Err("Vedalken Entrancer cast identity, {3}{U} payment, Defender, or 1/4 characteristics differ".to_owned());
    }
    if capture.ability_target != Some(Target::Player(PlayerId(1)))
        || capture.ability_cost != Some(ManaCost::with_colors(0, [Color::Blue]))
        || !capture.source_tapped_after_activation
        || capture.source_zone_after_resolution != Some(Zone::Battlefield)
        || capture.activation_active_player != Some(PlayerId(1))
    {
        return Err(format!(
            "Entrancer must pay {{U}} and tap at instant speed to target opponent while remaining on battlefield: target={:?}, cost={:?}, tapped={}, zone={:?}, active={:?}",
            capture.ability_target,
            capture.ability_cost,
            capture.source_tapped_after_activation,
            capture.source_zone_after_resolution,
            capture.activation_active_player
        ));
    }
    if capture.expected_milled.len() != 2
        || capture.milled != capture.expected_milled
        || capture.milled_owners != [PlayerId(1), PlayerId(1)]
        || !capture
            .library_before
            .zip(capture.library_after)
            .is_some_and(|(before, after)| before.checked_sub(2) == Some(after))
    {
        return Err(format!(
            "Entrancer did not mill exactly the two top opposing library objects: expected={:?}, actual={:?}, owners={:?}, library={:?}->{:?}",
            capture.expected_milled,
            capture.milled,
            capture.milled_owners,
            capture.library_before,
            capture.library_after
        ));
    }
    if capture.attack_turns.len() < 3
        || capture.damage_to_opponent < 9
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err("Watchwolf did not continue through combat to win the full duel".to_owned());
    }
    let spell_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 4 && colors.contains(&Color::Blue))).ok_or("Entrancer {3}{U} spell-payment receipt missing")?;
    let spell_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Entrancer SpellCast receipt missing")?;
    let spell_resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Entrancer SpellResolved receipt missing")?;
    let entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Entrancer battlefield entry receipt missing")?;
    let ability_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: known, ability: ABILITY_ID, mana_cost } if *known == source && *mana_cost == ManaCost::with_colors(0, [Color::Blue]))).ok_or("Entrancer exact {U} activated-cost receipt missing")?;
    let activated = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, definition: CARD_ID, ability: ABILITY_ID, .. } if *known == source)).ok_or("Entrancer AbilityActivated receipt missing")?;
    let mill_indices: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if capture.milled.contains(card)).then_some(index))
        .collect();
    if mill_indices.len() != 2 {
        return Err(format!(
            "Entrancer must move exactly two top cards to graveyard, observed {} moves",
            mill_indices.len()
        ));
    }
    let resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability: ABILITY_ID, .. } if *known == source)).ok_or("Entrancer AbilityResolved receipt missing")?;
    let wolf = capture.watchwolf.ok_or("Watchwolf finisher was not cast")?;
    let wolf_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf)).ok_or("Watchwolf SpellCast receipt missing")?;
    let attack = event_index_after(&capture.events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("Watchwolf did not attack after mill resolution")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == wolf && *amount > 0)).ok_or("Watchwolf dealt no combat damage after mill resolution")?;
    let ended = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    if !(spell_paid < spell_cast
        && spell_cast < spell_resolved
        && spell_resolved < entered
        && entered < ability_paid
        && ability_paid < activated
        && activated < mill_indices[0]
        && mill_indices[0] < mill_indices[1]
        && mill_indices[1] < resolved
        && resolved < wolf_cast
        && wolf_cast < attack
        && attack < damage
        && damage < ended)
    {
        return Err(format!(
            "Entrancer ordered spell/ability/mill/combat trace differs: spell=({spell_paid},{spell_cast},{spell_resolved},{entered}), ability=({ability_paid},{activated},{mill_indices:?},{resolved}), combat=({wolf_cast},{attack},{damage},{ended})"
        ));
    }
    let projected = project_events(&capture.events);
    let include: BTreeSet<_> = [
        spell_paid,
        spell_cast,
        spell_resolved,
        entered,
        ability_paid,
        activated,
        mill_indices[0],
        mill_indices[1],
        resolved,
        wolf_cast,
        attack,
        damage,
        ended,
    ]
    .into_iter()
    .collect();
    let events: Vec<_> = include
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "trace_id": "rav-vedalken-entrancer-blue-target-mill-two-full-game",
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": capture.rejected,
        "entrancer": {
            "object": source.0,
            "collector_number": 74,
            "mana_cost": "{3}{U}",
            "mana_paid": capture.entrancer_spell_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "power": capture.entrancer_power,
            "toughness": capture.entrancer_toughness,
            "defender": capture.has_defender,
            "resolved": capture.entrancer_resolved
        },
        "activation": {
            "source_object": ability_source.0,
            "ability_id": ABILITY_ID,
            "mana_cost": "{U}",
            "mana_paid": format!("{:?}", capture.ability_cost),
            "tap_cost_paid": capture.source_tapped_after_activation,
            "active_player_at_activation": capture.activation_active_player.map(|player| player.0),
            "target_player": capture.ability_target.as_ref().and_then(|target| match target { Target::Player(player) => Some(player.0), _ => None }),
            "library_size_before": capture.library_before,
            "library_size_after": capture.library_after,
            "expected_top_objects": capture.expected_milled.iter().map(|id| id.0).collect::<Vec<_>>(),
            "milled_objects": capture.milled.iter().map(|id| id.0).collect::<Vec<_>>(),
            "milled_owners": capture.milled_owners.iter().map(|player| player.0).collect::<Vec<_>>(),
            "source_zone_after_resolution": format!("{:?}", capture.source_zone_after_resolution)
        },
        "combat": {
            "card": "Watchwolf",
            "object": wolf.0,
            "attack_turns": capture.attack_turns,
            "damage_to_opponent": capture.damage_to_opponent,
            "life_at_terminal": capture.life
        },
        "ordered_event_sequence": [spell_paid, spell_cast, spell_resolved, entered, ability_paid, activated, mill_indices[0], mill_indices[1], resolved, wolf_cast, attack, damage, ended],
        "events": events,
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Vedalken Entrancer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "tap-blue-target-player-mill-two",
        ],
        power: Some(1),
        toughness: Some(4),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

fn expected_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: ABILITY_ID,
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Player],
        effects: vec![Effect::MillTargetPlayer { count: 2 }],
    }
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        .is_err())
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only",
        },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" },
        "credited": false
    })
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
