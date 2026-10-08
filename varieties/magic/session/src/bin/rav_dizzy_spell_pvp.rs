//! Full-game diagnostic authority for RAV Dizzy Spell's instant and Transmute.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList, DecisionKind,
    DecisionVisibility, Effect, Game, GameEvent, GameView, Keyword, Layer,
    LibrarySearchDestination, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DIZZY-SPELL";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const BLOCKER_ID: &str = "RAV-CARVEN-CARYATID";
const PILOT_ISLAND_ID: &str = "RAV-ISLAND";
const PILOT_FOREST_ID: &str = "RAV-FOREST";
const OPPONENT_FOREST_ID: &str = "RAV-FOREST";
const OPPONENT_PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dizzy_spell.rs";
const TRANSMUTE_ID: &str = "transmute";
const REFERENCE_SEED: u64 = 32;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dizzy_spell.rs"]
mod candidate_dizzy_spell;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dizzy Spell result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Dizzy Spell PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPower,
    WrongToughness,
    TransmuteRemoved,
    WrongTransmuteCost,
    WrongSpellCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    transmute_source: Option<ObjectId>,
    transmute_source_incarnation: Option<u64>,
    transmute_resolved_incarnation: Option<u64>,
    transmute_cost: Option<ManaCost>,
    transmute_search_controller_view: bool,
    transmute_hidden_from_opponent: bool,
    transmute_selection: Option<ObjectId>,
    transmute_found: Option<ObjectId>,
    transmute_found_definition: Option<&'static str>,
    transmute_found_in_hand: bool,
    transmute_resolved: bool,
    spell_source: Option<ObjectId>,
    spell_payment: Vec<Color>,
    spell_resolved: bool,
    spell_zone_after: Option<Zone>,
    target: Option<ObjectId>,
    target_definition: Option<&'static str>,
    target_controller: Option<PlayerId>,
    target_owner: Option<PlayerId>,
    target_power_before: Option<i32>,
    target_toughness_before: Option<i32>,
    target_power_after: Option<i32>,
    target_toughness_after: Option<i32>,
    target_zone_after: Option<Zone>,
    target_incarnation_before: Option<u64>,
    target_incarnation_after: Option<u64>,
    effect_created: bool,
    spell_turn: Option<u32>,
    attack_turn: Option<u32>,
    attack_event_index: Option<usize>,
    target_attacked: bool,
    target_damage_to_pilot_this_combat: i32,
    private_search_decision: Option<u64>,
    pilot_watchwolf: Option<ObjectId>,
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
        return Err("Dizzy Spell full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "minus_three_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower)?,
        "zero_toughness_modifier_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness)?,
        "transmute_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::TransmuteRemoved)?,
        "transmute_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTransmuteCost)?,
        "printed_spell_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongSpellCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "minus_three_power_mutation_detected_in_game",
        "zero_toughness_modifier_mutation_detected_in_game",
        "transmute_removed_mutation_detected_in_game",
        "transmute_cost_changed_mutation_detected_in_game",
        "printed_spell_cost_changed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Dizzy Spell gameplay mutant escaped: {key}"));
        }
    }
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings.iter().copied().filter(|card| names.insert(card.name)).collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": TASK_ID, "expansion": "RAV",
        "card": {"collector_number": 43, "printings": 1, "definition_id": CARD_ID, "name": "Dizzy Spell"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID}, "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["601.2: casting the instant spell", "117: instant-speed priority", "109.5 and 115: target identity and legality", "608.2: resolving a targeted spell", "613.4: power/toughness layer", "602: activated abilities", "702.53: Transmute", "701.19: library search and shuffle"],
            "source_contract": "RAV Dizzy Spell #43 is a blue instant costing {U}; target creature gets -3/-0 until end of turn; Transmute {1}{U}{U}.",
            "authority_cutoff": "2026-09-24", "no_provider_trial": true
        },
        "passed": true, "pilot_card_passed": true, "deterministic_replay": true,
        "cards_passed": 1, "cards_credited": 0, "cards_total": catalog.len(), "printings_total": printings.len(),
        "coverage_complete": false, "full_reward": false, "harbor_reward": 0.0, "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations, "traces": [trace], "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_dizzy_spell::definition();
    match mutation {
        Mutation::WrongPower => definition.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn { power: -2, toughness: 0 }],
        Mutation::WrongToughness => definition.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn { power: -3, toughness: -1 }],
        Mutation::TransmuteRemoved => definition.keywords.retain(|keyword| !matches!(keyword, Keyword::Transmute(_))),
        Mutation::WrongTransmuteCost => definition.keywords = vec![Keyword::Transmute(ManaCost::with_colors(2, [Color::Blue, Color::Blue]))],
        Mutation::WrongSpellCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Blue]),
        Mutation::None => {}
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_dizzy_spell::definition();
    let registered = card_definitions().into_iter().find(|card| card.id == CARD_ID)
        .ok_or("Dizzy Spell is missing from the RAV definition registry")?;
    if registered != definition || definition != expected_definition() {
        return Err("Dizzy Spell candidate definition differs from the RAV source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Dizzy Spell source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Dizzy Spell source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) || file.items.len() != 3 {
        return Err("Dizzy Spell module must contain private imports and one direct-literal definition function".to_owned());
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {
                imports += 1;
                if !safe_use_tree(&item.tree) { return Err("Dizzy Spell contains an unsupported import".to_owned()); }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || !function.attrs.iter().all(|attribute| attribute.path().is_ident("must_use") || attribute.path().is_ident("doc"))
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty() || function.sig.asyncness.is_some() || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty() || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && path_is(&literal.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>(), &["CardDefinition"]) && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(format!("Dizzy Spell function `{name}` is not a direct declarative card literal"));
                }
            }
            _ => return Err("Dizzy Spell source contains an executable or unsupported item".to_owned()),
        }
    }
    if imports != 2 || functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Dizzy Spell source imports or function identity differs from the source contract".to_owned());
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => matches!(path.ident.to_string().as_str(), "std" | "collections" | "cardbench_magic_engine") && safe_use_tree(&path.tree),
        UseTree::Name(name) => matches!(name.ident.to_string().as_str(), "BTreeSet" | "CardDefinition" | "CardType" | "Color" | "Effect" | "Keyword" | "ManaCost"),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(literal.lit, syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)),
        Expr::Path(path) => {
            let segments: Vec<_> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            path_is(&segments, &["None"]) || path_is(&segments, &["Color", "Blue"]) || path_is(&segments, &["CardType", "Instant"])
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return false; };
            let segments: Vec<_> = path.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            (path_is(&segments, &["ManaCost", "with_colors"]) || path_is(&segments, &["BTreeSet", "from"]) || path_is(&segments, &["BTreeSet", "new"]) || path_is(&segments, &["Keyword", "Transmute"]))
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            let segments: Vec<_> = literal.path.segments.iter().map(|segment| segment.ident.to_string()).collect();
            (path_is(&segments, &["CardDefinition"]) || path_is(&segments, &["Effect", "ModifyTargetPtUntilEndOfTurn"]) || path_is(&segments, &["Keyword", "Transmute"]))
                && literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => matches!(&*unary.expr, Expr::Lit(literal) if matches!(literal.lit, syn::Lit::Int(_))),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated.parse2(mac.mac.tokens.clone()).is_ok_and(|values| values.iter().all(safe_expression)),
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len() && actual.iter().zip(expected).all(|(actual, expected)| actual == expected)
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Dizzy Spell",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier", "transmute"],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue]))],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn { power: -3, toughness: 0 }],
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") { "agent" } else { "reference" }
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList { mainboard: entries.iter().map(|(card, count)| DeckEntry { card: (*card).to_owned(), count: *count }).collect(), sideboard: vec![] }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions.iter_mut().find(|definition| definition.id == CARD_ID).ok_or("Dizzy Spell definition is missing from RAV registry")? = candidate_definition(mutation);
    Game::new_with_all_bindings_and_triggers(
        definitions, 2, rav_mana_ability_bindings(), rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(), rav_activated_ability_bindings(), rav_triggered_ability_bindings(),
    ).map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_DIZZY_SPELL_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_DIZZY_SPELL_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    match run_match(REFERENCE_SEED, Mutation::None).and_then(|capture| validate_trace(&capture, REFERENCE_SEED).map(|_| capture)) {
        Ok(capture) => Ok((REFERENCE_SEED, capture)),
        Err(error) => Err(format!("Dizzy Spell reference seed {REFERENCE_SEED} did not meet the full-game contract: {error}")),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (TARGET_ID, 4), (BLOCKER_ID, 4), (PILOT_ISLAND_ID, 20), (PILOT_FOREST_ID, 20), (OPPONENT_PLAINS_ID, 8)]);
    let opponent = deck(&[(TARGET_ID, 4), (OPPONENT_FOREST_ID, 28), (OPPONENT_PLAINS_ID, 28)]);
    for (name, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list.mainboard.iter().map(|entry| usize::from(entry.count)).sum::<usize>() != 60 {
            return Err(format!("Dizzy Spell {name} fixture is not a legal 60-card deck"));
        }
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig { shuffle_seed: seed, max_turns: MAX_TURNS, max_policy_moves: MAX_MOVES, ..DeckMatchConfig::default() };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list).map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size).map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DizzyPolicy::new(PlayerId(0), mutation)),
        Box::new(DizzyPolicy::new(PlayerId(1), Mutation::None)),
    ];
    let mut capture = Captured {
        events: vec![], winner: None, turns: 0, accepted: 0, rejected: 0,
        transmute_source: None, transmute_source_incarnation: None, transmute_resolved_incarnation: None,
        transmute_cost: None, transmute_search_controller_view: false, transmute_hidden_from_opponent: true,
        transmute_selection: None, transmute_found: None, transmute_found_definition: None,
        transmute_found_in_hand: false, transmute_resolved: false,
        spell_source: None, spell_payment: vec![], spell_resolved: false, spell_zone_after: None,
        target: None, target_definition: None, target_controller: None, target_owner: None,
        target_power_before: None, target_toughness_before: None, target_power_after: None,
        target_toughness_after: None, target_zone_after: None, target_incarnation_before: None, target_incarnation_after: None,
        effect_created: false, spell_turn: None, attack_turn: None, attack_event_index: None, target_attacked: false,
        target_damage_to_pilot_this_combat: 0, private_search_decision: None,
        pilot_watchwolf: None, attack_turns: vec![], damage_to_opponent: 0, life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!("Dizzy Spell duel exceeded action bound at turn {} with life {:?}", game.turn, game.players.iter().map(|player| player.life).collect::<Vec<_>>()));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("Dizzy Spell policy seat missing")?;
        let action = if let Some(search) = &view.library_search_choice {
            if player == PlayerId(0) && Some(search.source) == capture.transmute_source {
                capture.transmute_search_controller_view = true;
                capture.private_search_decision = Some(search.decision.0);
                let hidden = game.view_for_player(PlayerId(1)).map_err(|error| error.to_string())?;
                capture.transmute_hidden_from_opponent &= hidden.library_search_choice.is_none();
            }
            policy.propose_library_search_choice(&view)
        } else if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        if let PolicyAction::Transmute { card } = &action
            && capture.transmute_source.is_none()
        {
            capture.transmute_source = Some(*card);
            capture.transmute_source_incarnation = game.object(*card).ok().map(|object| object.incarnation);
        }
        if let PolicyAction::ChooseLibrarySearchCard { source, selected, .. } = &action
            && Some(*source) == capture.transmute_source
        {
            capture.transmute_selection = *selected;
        }
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && capture.spell_source.is_none()
            && game.card_definition(request.card).is_ok_and(|definition| definition.id == CARD_ID)
        {
            capture.spell_source = Some(request.card);
            if let Some(Target::Permanent(target)) = request.targets.first() {
                capture.target = Some(*target);
                let object = game.object(*target).map_err(|error| error.to_string())?;
                let definition = game.card_definition(*target).map_err(|error| error.to_string())?;
                let characteristics = game.characteristics(*target).map_err(|error| error.to_string())?;
                capture.target_definition = Some(definition.id);
                capture.target_controller = Some(object.controller);
                capture.target_owner = Some(object.owner);
                capture.target_power_before = characteristics.power;
                capture.target_toughness_before = characteristics.toughness;
                capture.target_incarnation_before = Some(object.incarnation);
                capture.attack_event_index = game.event_log.iter().enumerate().rev().find_map(|(index, event)| {
                    matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(target)).then_some(index)
                });
                if capture.attack_event_index.is_some() {
                    capture.attack_turn = Some(game.turn);
                    capture.target_attacked = true;
                }
            }
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("Dizzy Spell move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        capture.accepted += 1;
        game.validate_invariants().map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [
        game.player(PlayerId(0)).map_err(|error| error.to_string())?.life,
        game.player(PlayerId(1)).map_err(|error| error.to_string())?.life,
    ];
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: TRANSMUTE_ID, mana_cost }
                if Some(*source) == capture.transmute_source => capture.transmute_cost = Some(mana_cost.clone()),
            GameEvent::AbilityActivated { player: PlayerId(0), source, source_incarnation, definition, ability: TRANSMUTE_ID }
                if Some(*source) == capture.transmute_source && *definition == CARD_ID => capture.transmute_source_incarnation = Some(*source_incarnation),
            GameEvent::AbilityResolved { source, source_incarnation, ability: TRANSMUTE_ID }
                if Some(*source) == capture.transmute_source => {
                    capture.transmute_resolved = true;
                    capture.transmute_resolved_incarnation = Some(*source_incarnation);
                }
            GameEvent::CardRevealed { player: PlayerId(0), card, definition }
                if Some(*card) == capture.transmute_selection => capture.transmute_found_definition = Some(definition),
            GameEvent::CardMoved { card, to: Zone::Hand } if Some(*card) == capture.transmute_selection => {
                capture.transmute_found = Some(*card);
                capture.transmute_found_in_hand = true;
            }
            GameEvent::LibrarySearchResolved { player: PlayerId(0), source, found, destination: LibrarySearchDestination::Hand }
                if Some(*source) == capture.transmute_source => capture.transmute_found = *found,
            GameEvent::Transmuted { player: PlayerId(0), discarded, found }
                if Some(*discarded) == capture.transmute_source => capture.transmute_found = *found,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                if Some(*card) == capture.spell_source => {
                    capture.spell_payment = colors.clone();
                    capture.spell_turn = Some(game.turn);
                }
            GameEvent::SpellResolved { card } if Some(*card) == capture.spell_source => capture.spell_resolved = true,
            GameEvent::CardMoved { card, to } if Some(*card) == capture.spell_source => capture.spell_zone_after = Some(*to),
            GameEvent::ContinuousEffectCreated { source, target, layer: Layer::PowerToughness }
                if Some(*source) == capture.spell_source && Some(*target) == capture.target => {
                    capture.effect_created = true;
                    let object = game.object(*target).map_err(|error| error.to_string())?;
                    let characteristics = game.characteristics(*target).map_err(|error| error.to_string())?;
                    capture.target_power_after = characteristics.power;
                    capture.target_toughness_after = characteristics.toughness;
                    capture.target_zone_after = game.zone_of(*target);
                    capture.target_incarnation_after = Some(object.incarnation);
                }
            GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if capture.spell_turn.is_none() && capture.target.is_some_and(|target| attackers.contains(&target)) => {
                capture.attack_turn = Some(game.turn);
                capture.target_attacked = true;
            }
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount }
                if Some(*source) == capture.target && capture.spell_turn == Some(game.turn) && capture.attack_turn == Some(game.turn) => capture.target_damage_to_pilot_this_combat += *amount,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if game.object(*card).is_ok_and(|object| object.owner == PlayerId(0)) && game.card_definition(*card).is_ok_and(|definition| definition.id == TARGET_ID) => {
                capture.pilot_watchwolf.get_or_insert(*card);
            }
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if capture.pilot_watchwolf.is_some_and(|source| attackers.contains(&source)) => capture.attack_turns.push(game.turn),
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if capture.pilot_watchwolf == Some(*source) => capture.damage_to_opponent += *amount,
            _ => {}
        }
    }
    Ok(())
}

struct DizzyPolicy {
    player: PlayerId,
    mutation: Mutation,
    transmute_started: bool,
    spell_cast: bool,
    creature_cast: bool,
    blocker_cast: bool,
    found_spell: Option<ObjectId>,
    spell_cast_turn: Option<u32>,
}

impl DizzyPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self { player, mutation, transmute_started: false, spell_cast: false, creature_cast: false, blocker_cast: false, found_spell: None, spell_cast_turn: None }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL.into_iter().map(|color| (color, view.mana_pool.amount(color))).collect();
        for color in &cost.colored {
            let amount = &mut available.iter_mut().find(|(known, _)| known == color)?.1;
            if *amount == 0 { return None; }
            *amount -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
            if *amount == 0 { return None; }
            *amount -= 1;
            generic.push(*color);
        }
        Some(ManaPaymentSelection { generic, hybrid: vec![] })
    }

    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let mut missing = None;
        for color in &cost.colored {
            let required = cost.colored.iter().filter(|candidate| *candidate == color).count() as u8;
            if view.mana_pool.amount(*color) < required { missing = Some(*color); break; }
        }
        if missing.is_none() && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8) {
            missing = Some(if view.own_battlefield.iter().any(|card| card.mana_colors.contains(&Color::Blue) && !card.tapped) { Color::Blue } else { Color::Green });
        }
        let needed = missing?;
        let land = view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && card.mana_colors.contains(&needed))
            .or_else(|| view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && !card.mana_colors.is_empty()))?;
        let color = if land.mana_colors.contains(&needed) { needed } else { *land.mana_colors.iter().next()? };
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 { return None; }
        let preferred = if self.player == PlayerId(0) {
            let blue_sources = view.own_battlefield.iter().filter(|card| card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Blue)).count();
            if blue_sources < 2 { PILOT_ISLAND_ID } else { PILOT_FOREST_ID }
        } else {
            let has_green = view.own_battlefield.iter().any(|card| card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Green));
            if has_green { OPPONENT_PLAINS_ID } else { OPPONENT_FOREST_ID }
        };
        view.hand.iter().find(|card| card.definition == Some(preferred))
            .or_else(|| view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast_spell(&mut self, view: &GameView, target: ObjectId, cost: &ManaCost) -> Option<PolicyAction> {
        let Some(card) = view.hand.iter().find(|card| card.definition == Some(CARD_ID) && (self.player != PlayerId(0) || self.found_spell == Some(card.id))) else { return None; };
        if let Some(mana_selection) = Self::payment(view, cost) {
            self.spell_cast = true;
            self.spell_cast_turn = Some(view.turn);
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest { card: card.id, targets: vec![Target::Permanent(target)], convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection,
            });
        }
        Self::make_mana(view, cost)
    }

    fn cast_watchwolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        let card = view.hand.iter().find(|card| card.definition == Some(TARGET_ID))?;
        if let Some(mana_selection) = Self::payment(view, &cost) {
            self.creature_cast = true;
            Some(PolicyAction::CastWithPayment {
                request: CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, &cost)
        }
    }

    fn cast_blocker(&mut self, view: &GameView) -> Option<PolicyAction> {
        let cost = ManaCost::with_colors(1, [Color::Green, Color::Green]);
        let card = view.hand.iter().find(|card| card.definition == Some(BLOCKER_ID))?;
        if let Some(mana_selection) = Self::payment(view, &cost) {
            self.blocker_cast = true;
            Some(PolicyAction::CastWithPayment {
                request: CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, &cost)
        }
    }
}

impl CodePolicy for DizzyPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) { "rav.dizzy-spell-transmute-and-combat.v1" } else { "rav.watchwolf-attacks-no-blocks.v1" }
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view.library_search_choice.as_ref().expect("visible library search choice");
        let selected = choice.cards.iter().find(|card| card.definition == Some(CARD_ID)).map(|card| card.id);
        if self.player == PlayerId(0) { self.found_spell = selected; }
        PolicyAction::ChooseLibrarySearchCard { decision: choice.decision, source: choice.source, selected }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            let attackers = if self.player == PlayerId(1) {
                view.own_battlefield.iter().filter(|card| card.definition == Some(TARGET_ID) && card.can_attack).map(|card| card.id).collect()
            } else {
                view.own_battlefield.iter().filter(|card| card.definition == Some(TARGET_ID) && card.can_attack).map(|card| card.id).collect()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if self.player == PlayerId(0)
            && view.active_player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.priority == self.player
            && view.stack_depth == 0
            && !self.spell_cast
        {
            if let Some(target) = view.opponent_battlefield.iter()
                .find(|card| card.definition == Some(TARGET_ID) && card.card_types.contains(&CardType::Creature))
                .map(|card| card.id)
            {
                let cost = candidate_definition(self.mutation).mana_cost;
                if let Some(action) = self.cast_spell(view, target, &cost) { return action; }
            }
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared {
            let spell_ready = !self.spell_cast && self.found_spell.is_some()
                && (view.mana_pool.amount(Color::Blue) > 0
                    || view.own_battlefield.iter().any(|card| card.card_types.contains(&CardType::Land) && !card.tapped && card.mana_colors.contains(&Color::Blue)));
            let current_spell_combat = self.spell_cast_turn == Some(view.turn);
            let assignments = if self.player == PlayerId(0) && !current_spell_combat && !spell_ready {
                view.combat_attackers.iter()
                    .find(|attacker| attacker.definition == Some(TARGET_ID))
                    .and_then(|attacker| view.own_battlefield.iter().find(|blocker| blocker.definition == Some(BLOCKER_ID) && blocker.can_block && !blocker.tapped).map(|blocker| CombatBlock { attacker: attacker.id, blocker: blocker.id }))
                    .into_iter().collect()
            } else { vec![] };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player || view.stack_depth != 0 { return PolicyAction::PassPriority; }
        if view.active_player == self.player && view.step == Step::PrecombatMain {
            if let Some(action) = self.play_land(view) { return action; }
            if self.player == PlayerId(0) {
                if !self.blocker_cast
                    && !view.own_battlefield.iter().any(|card| card.definition == Some(BLOCKER_ID))
                    && let Some(action) = self.cast_blocker(view)
                { return action; }
                let definition = candidate_definition(self.mutation);
                if !self.transmute_started
                    && let Some(cost) = definition.transmute_cost()
                    && let Some(card) = view.hand.iter().find(|card| card.definition == Some(CARD_ID))
                {
                    if let Some(_payment) = Self::payment(view, cost) {
                        self.transmute_started = true;
                        return PolicyAction::Transmute { card: card.id };
                    }
                    if let Some(action) = Self::make_mana(view, cost) { return action; }
                }
                if self.spell_cast && !self.creature_cast
                    && !view.own_battlefield.iter().any(|card| card.definition == Some(TARGET_ID))
                    && let Some(action) = self.cast_watchwolf(view)
                { return action; }
            } else if !self.creature_cast && !view.own_battlefield.iter().any(|card| card.definition == Some(TARGET_ID))
                && let Some(action) = self.cast_watchwolf(view)
            { return action; }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected != 0 {
        return Err(format!("Dizzy Spell duel did not reach a clean P0 terminal win: winner={:?}, turns={}, rejected={}, transmute={:?}, found={:?}, spell={:?}, target={:?}, spell_turn={:?}, attack_turn={:?}, damage={}, life={:?}", capture.winner, capture.turns, capture.rejected, capture.transmute_source, capture.transmute_found, capture.spell_source, capture.target, capture.spell_turn, capture.attack_turn, capture.damage_to_opponent, capture.life));
    }
    let transmute_source = capture.transmute_source.ok_or("Dizzy Spell Transmute was not activated")?;
    let found = capture.transmute_found.ok_or("Dizzy Spell Transmute did not find a card")?;
    let spell = capture.spell_source.ok_or("Dizzy Spell was not cast")?;
    let target = capture.target.ok_or("Dizzy Spell did not select a creature")?;
    let expected_transmute = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
    if transmute_source == spell || found != spell || capture.transmute_selection != Some(found)
        || capture.transmute_found_definition != Some(CARD_ID) || !capture.transmute_found_in_hand
        || !capture.transmute_resolved || capture.transmute_source_incarnation.is_none()
        || capture.transmute_source_incarnation != capture.transmute_resolved_incarnation
        || capture.transmute_cost.as_ref() != Some(&expected_transmute)
        || !capture.transmute_search_controller_view || !capture.transmute_hidden_from_opponent
    {
        return Err("Dizzy Spell Transmute cost, private selection, exact found object, or source-incarnation lifecycle differs".to_owned());
    }
    let blue_mana_activations_before_transmute = capture.events.iter().take_while(|event| {
        !matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: TRANSMUTE_ID, .. } if *source == transmute_source)
    }).filter(|event| matches!(event, GameEvent::ManaAbilityActivated { player: PlayerId(0), color: Color::Blue, .. })).count();
    if blue_mana_activations_before_transmute < 2 {
        return Err("Dizzy Spell Transmute did not pay its two blue symbols from mana abilities".to_owned());
    }
    if capture.spell_payment != vec![Color::Blue] || !capture.spell_resolved
        || capture.spell_zone_after != Some(Zone::Graveyard) || !capture.effect_created
        || capture.target_definition != Some(TARGET_ID)
        || capture.target_controller != Some(PlayerId(1)) || capture.target_owner != Some(PlayerId(1))
        || capture.target_power_before != Some(3) || capture.target_toughness_before != Some(3)
        || capture.target_power_after != Some(0) || capture.target_toughness_after != Some(3)
        || capture.target_zone_after != Some(Zone::Battlefield)
        || capture.target_incarnation_before != capture.target_incarnation_after
        || !capture.target_attacked || capture.attack_turn.is_none()
        || capture.spell_turn != capture.attack_turn
        || capture.target_damage_to_pilot_this_combat != 0
    {
        return Err(format!(
            "Dizzy Spell did not resolve from its {{U}} cost against the exact attacking 3/3 Watchwolf for -3/-0 with no damage: target={target:?}, P/T={:?}/{:?}->{:?}/{:?}, zone={:?}, same-combat damage={}",
            capture.target_power_before, capture.target_toughness_before, capture.target_power_after,
            capture.target_toughness_after, capture.target_zone_after, capture.target_damage_to_pilot_this_combat
        ));
    }
    if capture.attack_turns.len() < 5 || capture.damage_to_opponent < 15
        || capture.life[0] <= 0 || capture.life[1] > 0
    {
        return Err("Dizzy Spell duel did not continue through combat to a terminal P0 win".to_owned());
    }

    let paid = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: TRANSMUTE_ID, mana_cost } if *source == transmute_source && *mana_cost == expected_transmute)).ok_or("Dizzy Spell exact {1}{U}{U} AbilityManaPaid receipt missing")?;
    let discarded = unique_index(&capture.events, |event| matches!(event, GameEvent::DiscardedAsAbilityCost { player: PlayerId(0), source, card } if *source == transmute_source && *card == transmute_source)).ok_or("Dizzy Spell exact Transmute discard-cost receipt missing")?;
    let transmute_moved = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == transmute_source)).ok_or("Dizzy Spell Transmute source did not move to its owner's graveyard")?;
    let activated = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: TRANSMUTE_ID, definition: CARD_ID, .. } if *source == transmute_source)).ok_or("Dizzy Spell AbilityActivated receipt missing")?;
    let decision_id = capture.private_search_decision.ok_or("Dizzy Spell private library-search decision was not observed")?;
    let opened = unique_index(&capture.events, |event| matches!(event, GameEvent::DecisionOpened { decision, player: PlayerId(0), kind: DecisionKind::LibrarySearch, visibility: DecisionVisibility::Private, .. } if decision.0 == decision_id)).ok_or("Dizzy Spell controller-private Transmute search decision receipt missing")?;
    let completed = unique_index(&capture.events, |event| matches!(event, GameEvent::DecisionCompleted { decision, player: PlayerId(0), kind: DecisionKind::LibrarySearch } if decision.0 == decision_id)).ok_or("Dizzy Spell private Transmute decision completion receipt missing")?;
    let revealed = unique_index(&capture.events, |event| matches!(event, GameEvent::CardRevealed { player: PlayerId(0), card, definition: CARD_ID } if *card == found)).ok_or("Dizzy Spell Transmute found card reveal missing")?;
    let found_moved = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == found)).ok_or("Dizzy Spell Transmute exact result did not move to hand")?;
    let search = unique_index(&capture.events, |event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source, found: Some(selected), destination: LibrarySearchDestination::Hand } if *source == transmute_source && *selected == found)).ok_or("Dizzy Spell exact library-search receipt missing")?;
    let shuffled = event_index_after(&capture.events, search, |event| matches!(event, GameEvent::LibraryShuffled { player: PlayerId(0), .. })).ok_or("Dizzy Spell Transmute shuffle receipt missing")?;
    let transmuted = unique_index(&capture.events, |event| matches!(event, GameEvent::Transmuted { player: PlayerId(0), discarded, found: Some(selected) } if *discarded == transmute_source && *selected == found)).ok_or("Dizzy Spell Transmuted exact source/found receipt missing")?;
    let transmute_resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: TRANSMUTE_ID } if *source == transmute_source && Some(*source_incarnation) == capture.transmute_source_incarnation)).ok_or("Dizzy Spell Transmute AbilityResolved source-incarnation receipt missing")?;
    let attack_turn = capture.attack_turn.ok_or("opposing Watchwolf did not attack")?;
    let attack = capture.attack_event_index.filter(|index| matches!(capture.events.get(*index), Some(GameEvent::AttackersDeclared { player: PlayerId(1), attackers }) if attackers.contains(&target))).ok_or("target Watchwolf attacker receipt missing")?;
    let spell_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors == &[Color::Blue])).ok_or("Dizzy Spell exact blue payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Dizzy Spell cast receipt missing")?;
    let effect = unique_index(&capture.events, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: selected, layer: Layer::PowerToughness } if *source == spell && *selected == target)).ok_or("Dizzy Spell exact layer-seven effect receipt missing")?;
    let resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell)).ok_or("Dizzy Spell SpellResolved receipt missing")?;
    let terminal_zone = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("Dizzy Spell terminal graveyard move missing")?;
    let combat_damage_step = event_index_after(&capture.events, terminal_zone, |event| matches!(event, GameEvent::StepBegan { turn, step: Step::CombatDamage, .. } if *turn == attack_turn)).ok_or("targeted attack did not continue to the combat-damage step")?;
    let ended = unique_index(&capture.events, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal P0 GameEnded receipt missing")?;
    let sequence = [paid, discarded, transmute_moved, activated, opened, completed, revealed, found_moved, search, shuffled, transmuted, transmute_resolved, attack, spell_paid, cast, effect, resolved, terminal_zone, combat_damage_step, ended];
    if !sequence.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(format!("Dizzy Spell Transmute/cast/combat event order differs: {sequence:?}"));
    }
    let projected = project_events(&capture.events);
    let events: Vec<_> = sequence.iter().map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string())).collect::<Result<_, _>>()?;
    Ok(json!({
        "trace_id": "rav-dizzy-spell-transmute-and-minus-three-power-full-game", "seed": seed,
        "terminal": "Winner(PlayerId(0))", "winner_seat": 0, "turns": capture.turns,
        "accepted_moves": capture.accepted, "rejected_moves": capture.rejected,
        "transmute": {"source_object": transmute_source.0, "source_incarnation": capture.transmute_source_incarnation, "resolved_source_incarnation": capture.transmute_resolved_incarnation, "ability_id": TRANSMUTE_ID, "mana_cost": "{1}{U}{U}", "generic": capture.transmute_cost.as_ref().map(|cost| cost.generic), "colored": capture.transmute_cost.as_ref().map(|cost| cost.colored.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()), "search_decision": decision_id, "controller_saw_private_candidates": capture.transmute_search_controller_view, "opponent_could_not_see_candidates": capture.transmute_hidden_from_opponent, "selected_object": found.0, "selected_definition": capture.transmute_found_definition, "selected_to_hand": capture.transmute_found_in_hand, "same_object_cast": found == spell, "resolved": capture.transmute_resolved},
        "spell": {"object": spell.0, "collector_number": 43, "mana_cost": "{U}", "mana_paid": capture.spell_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "resolved": capture.spell_resolved, "terminal_zone": format!("{:?}", capture.spell_zone_after)},
        "target": {"object": target.0, "definition": capture.target_definition, "controller": capture.target_controller.map(|player| player.0), "owner": capture.target_owner.map(|player| player.0), "was_attacking_when_targeted": capture.target_attacked, "spell_turn": capture.spell_turn, "attack_turn": capture.attack_turn, "power_before": capture.target_power_before, "toughness_before": capture.target_toughness_before, "power_after": capture.target_power_after, "toughness_after": capture.target_toughness_after, "zone_after": format!("{:?}", capture.target_zone_after), "incarnation_before": capture.target_incarnation_before, "incarnation_after": capture.target_incarnation_after, "damage_to_pilot_this_combat": capture.target_damage_to_pilot_this_combat, "damage_prevented": capture.target_damage_to_pilot_this_combat == 0, "layer_seven_effect_created": capture.effect_created},
        "combat": {"card": "Watchwolf", "object": capture.pilot_watchwolf.map(|object| object.0), "attack_turns": capture.attack_turns, "damage_to_opponent": capture.damage_to_opponent, "life_at_terminal": capture.life},
        "ordered_event_sequence": sequence, "events": events, "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation).and_then(|capture| validate_trace(&capture, seed).map(|_| capture)).is_err())
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events.iter().enumerate().filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn event_index_after(events: &[GameEvent], after: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" { return args.next().map(std::path::PathBuf::from); }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
