//! Full-game diagnostic authority for the RAV Sandsower task.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
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
const CARD_ID: &str = "RAV-SANDSOWER";
const ABILITY_ID: &str = "tap-target-creature";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const BIRDS_ID: &str = "RAV-BIRDS-OF-PARADISE";
const TRANSLUMINANT_ID: &str = "RAV-TRANSLUMINANT";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sandsower.rs";
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 50;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sandsower.rs"]
mod candidate_sandsower;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Sandsower result: {error}");
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
            eprintln!("Sandsower PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    SourceTapAdded,
    TapCountChanged,
    TapEffectRemoved,
    PrintedCostChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TappedPayer {
    object: ObjectId,
    definition: String,
    tapped_after_activation: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    sandsower: Option<ObjectId>,
    spell_payment: Vec<Color>,
    spell_resolved: bool,
    spell_entered: bool,
    power: Option<i32>,
    toughness: Option<i32>,
    source: Option<ObjectId>,
    source_tapped_after_activation: bool,
    target: Option<ObjectId>,
    target_definition: Option<String>,
    target_controller: Option<usize>,
    target_tapped_before: bool,
    target_tapped_after_activation: bool,
    target_tapped_after_resolution: bool,
    target_tap_events: u32,
    active_player_at_activation: Option<usize>,
    payers: Vec<TappedPayer>,
    payers_seen: Vec<ObjectId>,
    ability_activated: bool,
    ability_resolved: bool,
    opponent_watchwolf: Option<ObjectId>,
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
        return Err("Sandsower full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "source_tap_cost_added_mutation_detected_in_game": mutation_detected(seed, Mutation::SourceTapAdded)?,
        "additional_creature_tap_count_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::TapCountChanged)?,
        "tap_effect_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::TapEffectRemoved)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::PrintedCostChanged)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "source_tap_cost_added_mutation_detected_in_game",
        "additional_creature_tap_count_changed_mutation_detected_in_game",
        "tap_effect_removed_mutation_detected_in_game",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Sandsower gameplay mutant escaped: {key}"));
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
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 28, "printings": 1, "definition_id": CARD_ID, "name": "Sandsower"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "302.1 and 302.6: creature permanents and summoning sickness",
                "602.2: activate an ability and pay its costs",
                "608.2: resolve a targeted activated ability",
                "701.21: tap a permanent",
                "508.1 and 510.1: attack declaration and combat damage"
            ],
            "source_contract": "RAV Sandsower #28 is a white 1/3 Creature with {3}{W} and 'Tap three untapped creatures you control: Tap target creature.' Its own tap is not a cost.",
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
        "qualification_status": "diagnostic_pilot_trace_passed; independent review pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_sandsower::definition();
    if mutation == Mutation::PrintedCostChanged {
        definition.mana_cost = ManaCost::with_colors(4, [Color::White]);
    }
    definition
}

fn candidate_ability(mutation: Mutation) -> ActivatedAbility {
    let mut ability = candidate_sandsower::activated_ability().ability;
    match mutation {
        Mutation::None | Mutation::PrintedCostChanged => {}
        Mutation::SourceTapAdded => ability.tap_cost = true,
        Mutation::TapCountChanged => ability.additional_tap_creatures = 2,
        Mutation::TapEffectRemoved => ability.effects.clear(),
    }
    ability
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_sandsower::definition();
    let binding = candidate_sandsower::activated_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sandsower is missing from the RAV definition registry")?;
    let matches: Vec<_> = rav_activated_ability_bindings()
        .into_iter()
        .filter(|candidate| candidate.card_definition == CARD_ID)
        .collect();
    if registered != definition
        || matches.len() != 1
        || matches[0] != binding
        || definition != expected_definition()
        || binding.ability != expected_ability()
    {
        return Err("Sandsower candidate definition or activated ability differs from the RAV source contract".to_owned());
    }
    Ok(())
}

/// The submitted module may contain only two direct struct-literal functions.
/// This source check runs before either candidate function is invoked.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sandsower candidate source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Sandsower source is not valid Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Sandsower module must contain only private imports and two literal functions".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if matches!(&item.vis, Visibility::Inherited) && safe_use_tree(&item.tree) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attr| attr.path().is_ident("must_use") || attr.path().is_ident("doc"))
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(format!("Sandsower function `{name}` is not a direct literal"));
                }
            }
            _ => return Err("Sandsower source contains executable or unsupported top-level code".to_owned()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()]) {
        return Err("Sandsower source function identity differs from the source contract".to_owned());
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => matches!(path.ident.to_string().as_str(), "std" | "collections" | "cardbench_magic_engine") && safe_use_tree(&path.tree),
        UseTree::Name(name) => matches!(name.ident.to_string().as_str(), "BTreeSet" | "ActivatedAbility" | "ActivatedAbilityBinding" | "CardDefinition" | "CardType" | "Color" | "Effect" | "ManaCost" | "TargetRequirement"),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(value.lit, syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)),
        Expr::Path(path) => {
            let path = path.path.segments.iter().map(|part| part.ident.to_string()).collect::<Vec<_>>();
            matches!(path.as_slice(), [a, b] if (a == "Color" && b == "White") || (a == "CardType" && b == "Creature") || (a == "TargetRequirement" && b == "Creature") || (a == "Effect" && b == "TapTargetCreature"))
                || path.as_slice() == ["None"]
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else { return false; };
            let path = path.path.segments.iter().map(|part| part.ident.to_string()).collect::<Vec<_>>();
            (matches!(path.as_slice(), [a, b] if (a == "ManaCost" && (b == "with_colors" || b == "new")) || (a == "BTreeSet" && (b == "from" || b == "new"))) || path.as_slice() == ["Some"])
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            let path = literal.path.segments.iter().map(|part| part.ident.to_string()).collect::<Vec<_>>();
            path.len() == 1 && matches!(path[0].as_str(), "CardDefinition" | "ActivatedAbility" | "ActivatedAbilityBinding")
                && literal.rest.is_none()
                && literal.fields.iter().all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
            .parse2(mac.mac.tokens.clone())
            .is_ok_and(|values| values.iter().all(safe_expression)),
        _ => false,
    }
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Sandsower",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-three-untapped-controlled-creatures",
            "tap-target-creature",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: ABILITY_ID,
        mana_cost: ManaCost::new(0),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 3,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::TapTargetCreature],
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions.iter_mut().find(|definition| definition.id == CARD_ID)
        .ok_or("Sandsower is missing from the RAV registry")? = candidate_definition(mutation);
    let mut abilities = rav_activated_ability_bindings();
    let matches: Vec<_> = abilities.iter_mut().filter(|binding| binding.card_definition == CARD_ID).collect();
    if matches.len() != 1 { return Err(format!("expected one Sandsower ability binding, found {}", matches.len())); }
    matches.into_iter().next().expect("one binding").ability = candidate_ability(mutation);
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    ).map_err(|error| error.to_string())
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList { mainboard: entries.iter().map(|(card, count)| DeckEntry { card: (*card).to_owned(), count: *count }).collect(), sideboard: vec![] }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SANDSOWER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| "CARDBENCH_SANDSOWER_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None).and_then(|capture| validate_trace(&capture, seed).map(|_| capture)) {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!("no complete Sandsower game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (BIRDS_ID, 4), (TRANSLUMINANT_ID, 4), (WATCHWOLF_ID, 4), (FOREST_ID, 20), (PLAINS_ID, 24)]);
    let opponent = deck(&[(WATCHWOLF_ID, 4), (FOREST_ID, 28), (PLAINS_ID, 28)]);
    if [&pilot, &opponent].iter().any(|list| list.mainboard.iter().map(|entry| usize::from(entry.count)).sum::<usize>() != 60) {
        return Err("Sandsower decks must each contain a legal 60-card mainboard".to_owned());
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig { shuffle_seed: seed, max_turns: MAX_TURNS, max_policy_moves: MAX_MOVES, ..DeckMatchConfig::default() };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list).map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size).map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(SandsowerPolicy::new(PlayerId(0), mutation)), Box::new(SandsowerPolicy::new(PlayerId(1), Mutation::None))];
    let mut capture = Captured {
        events: vec![], winner: None, turns: 0, accepted: 0, rejected: 0,
        sandsower: None, spell_payment: vec![], spell_resolved: false, spell_entered: false,
        power: None, toughness: None, source: None, source_tapped_after_activation: false,
        target: None, target_definition: None, target_controller: None, target_tapped_before: false,
        target_tapped_after_activation: false, target_tapped_after_resolution: false, target_tap_events: 0,
        active_player_at_activation: None, payers: vec![], payers_seen: vec![],
        ability_activated: false, ability_resolved: false, opponent_watchwolf: None,
        attack_turns: vec![], damage_to_opponent: 0, life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!("Sandsower duel exceeded its run bound on turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("Sandsower policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) { action }
            else if view.optional_triggered_ability_choice.is_some() { policy.propose_optional_triggered_ability(&view) }
            else if view.draw_replacement_pending { policy.propose_draw_replacement(&view) }
            else if view.private_library_choice.is_some() { policy.propose_private_library_choice(&view) }
            else if view.private_opponent_library_choice.is_some() { policy.propose_private_opponent_library_choice(&view) }
            else if view.library_search_choice.is_some() { policy.propose_library_search_choice(&view) }
            else { policy.propose_move(&view) };
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game.card_definition(request.card).is_ok_and(|definition| definition.id == CARD_ID) {
            capture.sandsower = Some(request.card);
        }
        let activation_snapshot = if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == ABILITY_ID {
            let target = activation.targets.first().and_then(|target| match target { Target::Permanent(id) => Some(*id), _ => None }).ok_or("Sandsower activation did not target a permanent")?;
            capture.source = Some(activation.source);
            capture.target = Some(target);
            capture.target_definition = game.card_definition(target).ok().map(|definition| definition.id.to_owned());
            capture.target_controller = game.controller_of(target).ok().map(|controller| controller.0);
            capture.target_tapped_before = game.object(target).is_ok_and(|object| object.tapped);
            capture.active_player_at_activation = Some(view.active_player.0);
            capture.payers = activation.additional_tap_creatures.iter().map(|object| TappedPayer {
                object: *object,
                definition: game.card_definition(*object).map(|definition| definition.id.to_owned()).unwrap_or_else(|_| "unknown".to_owned()),
                tapped_after_activation: false,
            }).collect();
            Some((activation.source, target, activation.additional_tap_creatures.clone()))
        } else { None };
        let event_cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("Sandsower policy move rejected at turn {} {:?}: {error}", view.turn, view.step))?;
        capture.accepted += 1;
        if let Some((source, target, payers)) = activation_snapshot {
            capture.source_tapped_after_activation = game.object(source).is_ok_and(|object| object.tapped);
            capture.target_tapped_after_activation = game.object(target).is_ok_and(|object| object.tapped);
            for payer in &mut capture.payers {
                payer.tapped_after_activation = game.object(payer.object).is_ok_and(|object| object.tapped);
            }
            capture.payers_seen = payers;
        }
        game.validate_invariants().map_err(|error| format!("post-move Sandsower invariant failure: {error}"))?;
        observe(&game, &game.event_log[event_cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [game.player(PlayerId(0)).map_err(|error| error.to_string())?.life, game.player(PlayerId(1)).map_err(|error| error.to_string())?.life];
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    let source = capture.sandsower;
    for event in events {
        match event {
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if Some(*card) == source => capture.spell_payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == source => capture.spell_resolved = true,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if Some(*card) == source => {
                capture.spell_entered = true;
                if let Ok(characteristics) = game.characteristics(*card) { capture.power = characteristics.power; capture.toughness = characteristics.toughness; }
            }
            GameEvent::SpellCast { player: PlayerId(1), card } if game.card_definition(*card).is_ok_and(|definition| definition.id == WATCHWOLF_ID) => capture.opponent_watchwolf = Some(*card),
            GameEvent::AbilityActivated { player: PlayerId(0), source, definition, ability, .. } if Some(*source) == capture.source && *definition == CARD_ID && *ability == ABILITY_ID => capture.ability_activated = true,
            GameEvent::AbilityResolved { source, ability, .. } if Some(*source) == capture.source && *ability == ABILITY_ID => {
                capture.ability_resolved = true;
                capture.target_tapped_after_resolution = capture.target.is_some_and(|target| game.object(target).is_ok_and(|object| object.tapped));
            }
            GameEvent::PermanentTapped { source, card } if Some(*source) == capture.source && Some(*card) == capture.target => capture.target_tap_events += 1,
            GameEvent::AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source, permanent } if Some(*source) == capture.source => { let _ = permanent; },
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|attacker| capture.sandsower != Some(*attacker)) => capture.attack_turns.push(game.turn),
            GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } => capture.damage_to_opponent += *amount,
            _ => {}
        }
    }
    Ok(())
}

struct SandsowerPolicy { player: PlayerId, mutation: Mutation, activation_offered: bool }

impl SandsowerPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self { Self { player, mutation, activation_offered: false } }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL.into_iter().map(|color| (color, view.mana_pool.amount(color))).collect();
        for color in &cost.colored { let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?; if *amount == 0 { return None; } *amount -= 1; }
        let mut generic = Vec::new();
        for _ in 0..cost.generic { let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?; if *amount == 0 { return None; } *amount -= 1; generic.push(*color); }
        Some(ManaPaymentSelection { generic, hybrid: vec![] })
    }

    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
        let land = view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && missing.is_some_and(|color| card.mana_colors.contains(&color)))
            .or_else(|| view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && !card.mana_colors.is_empty()))?;
        let color = missing.filter(|color| land.mana_colors.contains(color)).or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }

    fn play_land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 { return None; }
        view.hand.iter().find(|card| card.definition == Some(FOREST_ID)).or_else(|| view.hand.iter().find(|card| card.definition == Some(PLAINS_ID))).map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast(view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        if let Some(payment) = Self::payment(view, cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(id))?;
            Some(PolicyAction::CastWithPayment { request: CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }, chosen_x: None, mana_selection: payment })
        } else { Self::make_mana(view, cost) }
    }
}

impl CodePolicy for SandsowerPolicy {
    fn id(&self) -> &'static str { if self.player == PlayerId(0) { "rav.sandsower-pilot.v1" } else { "rav.sandsower-opponent.v1" } }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            let attackers = if self.player == PlayerId(0) && self.activation_offered { view.own_battlefield.iter().filter(|card| card.definition != Some(CARD_ID) && card.can_attack).map(|card| card.id).collect() } else { vec![] };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared { return PolicyAction::DeclareBlockers { assignments: vec![] }; }
        if view.priority != self.player || view.stack_depth != 0 { return PolicyAction::PassPriority; }

        if self.player == PlayerId(0) && !self.activation_offered && view.active_player != self.player {
            let source = view.own_battlefield.iter().find(|card| card.definition == Some(CARD_ID));
            let target = view.opponent_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID) && !card.tapped);
            if let (Some(source), Some(target)) = (source, target) {
                let payers: Vec<_> = view.own_battlefield.iter().filter(|card| card.id != source.id && card.card_types.contains(&CardType::Creature) && !card.tapped).take(3).collect();
                if payers.len() == 3 {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility { activation: AbilityActivation { source: source.id, ability_id: ABILITY_ID, sacrifice_sources: vec![], additional_tap_creatures: payers.iter().map(|card| card.id).collect(), discard_cards: vec![], targets: vec![Target::Permanent(target.id)] } };
                }
            }
        }

        if view.active_player == self.player && view.step.is_main() {
            if let Some(action) = Self::play_land(view) { return action; }
            if self.player == PlayerId(1) {
                let has_wolf = view.own_battlefield.iter().any(|card| card.definition == Some(WATCHWOLF_ID));
                if !has_wolf && view.hand.iter().any(|card| card.definition == Some(WATCHWOLF_ID)) {
                    let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                    if let Some(action) = Self::cast(view, WATCHWOLF_ID, &cost) { return action; }
                }
            } else {
                let creatures = view.own_battlefield.iter().filter(|card| card.definition != Some(CARD_ID) && card.card_types.contains(&CardType::Creature)).count();
                if creatures < 3 {
                    let support = [(BIRDS_ID, ManaCost::with_colors(0, [Color::Green])), (TRANSLUMINANT_ID, ManaCost::with_colors(1, [Color::Green])), (WATCHWOLF_ID, ManaCost::with_colors(0, [Color::Green, Color::White]))];
                    for (id, cost) in support {
                        if view.hand.iter().any(|card| card.definition == Some(id)) {
                            if let Some(action) = Self::cast(view, id, &cost) { return action; }
                        }
                    }
                } else if view.hand.iter().any(|card| card.definition == Some(CARD_ID)) && !view.own_battlefield.iter().any(|card| card.definition == Some(CARD_ID)) {
                    let cost = candidate_definition(self.mutation).mana_cost;
                    if let Some(action) = Self::cast(view, CARD_ID, &cost) { return action; }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected != 0 { return Err("Sandsower game did not reach a clean seat-zero terminal win".to_owned()); }
    let source = capture.sandsower.ok_or("Sandsower was not cast from the pilot deck")?;
    if !capture.spell_resolved || !capture.spell_entered || capture.power != Some(1) || capture.toughness != Some(3) {
        return Err("Sandsower did not resolve as a 1/3 battlefield creature".to_owned());
    }
    let paid_white = capture.spell_payment.iter().filter(|color| **color == Color::White).count();
    if capture.spell_payment.len() != 4 || paid_white != 1 { return Err(format!("Sandsower cast payment was not exactly {{3}}{{W}}: {:?}", capture.spell_payment)); }
    let ability_source = capture.source.ok_or("Sandsower activated ability was not used")?;
    let target = capture.target.ok_or("Sandsower activation target missing")?;
    if ability_source != source || capture.target_definition.as_deref() != Some(WATCHWOLF_ID) || capture.target_controller != Some(1)
        || capture.target_tapped_before || capture.target_tapped_after_activation || !capture.target_tapped_after_resolution
        || capture.target_tap_events != 1 || capture.source_tapped_after_activation || capture.active_player_at_activation != Some(1)
        || !capture.ability_activated || !capture.ability_resolved {
        return Err(format!("Sandsower must activate at instant speed without tapping itself, then tap the opposing Watchwolf only on resolution: target={target:?}, definition={:?}, controller={:?}, pre={}, after_activation={}, after_resolution={}, source_tapped={}, active={:?}, taps={}", capture.target_definition, capture.target_controller, capture.target_tapped_before, capture.target_tapped_after_activation, capture.target_tapped_after_resolution, capture.source_tapped_after_activation, capture.active_player_at_activation, capture.target_tap_events));
    }
    if capture.payers_seen.len() != 3 || capture.payers_seen.iter().copied().collect::<HashSet<_>>().len() != 3
        || capture.payers.len() != 3 || capture.payers.iter().any(|payer| !payer.tapped_after_activation || payer.object == source || payer.object == target) {
        return Err(format!("Sandsower must tap exactly three distinct untapped controlled creatures as costs: {:?}", capture.payers));
    }
    let cost_receipts: Vec<_> = capture.events.iter().filter_map(|event| match event {
        GameEvent::AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: known, permanent } if *known == source => Some(*permanent),
        _ => None,
    }).collect();
    if cost_receipts.len() != 3 || cost_receipts.iter().copied().collect::<HashSet<_>>().len() != 3 || cost_receipts.iter().any(|payer| !capture.payers_seen.contains(payer)) {
        return Err("Sandsower must emit exactly three matching additional creature cost receipts".to_owned());
    }
    if capture.attack_turns.is_empty() || capture.damage_to_opponent < 20 || capture.life[0] <= 0 || capture.life[1] > 0 {
        return Err("pilot did not continue the full game through combat to a terminal win".to_owned());
    }

    let paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == source)).ok_or("Sandsower spell payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Sandsower SpellCast receipt missing")?;
    let resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == source)).ok_or("Sandsower SpellResolved receipt missing")?;
    let entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Sandsower battlefield entry receipt missing")?;
    let wolf = capture.opponent_watchwolf.ok_or("opponent Watchwolf was not cast")?;
    let wolf_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == wolf)).ok_or("opponent Watchwolf cast receipt missing")?;
    let wolf_resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellResolved { card } if *card == wolf)).ok_or("opponent Watchwolf resolution receipt missing")?;
    let wolf_entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf)).ok_or("opponent Watchwolf entry receipt missing")?;
    let activated = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, definition: CARD_ID, ability: ABILITY_ID, .. } if *known == source)).ok_or("Sandsower AbilityActivated receipt missing")?;
    let tap_indices: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AdditionalCreatureTappedAsAbilityCost { player: PlayerId(0), source: known, .. } if *known == source).then_some(index)).collect();
    let target_tap = unique_index(&capture.events, |event| matches!(event, GameEvent::PermanentTapped { source: known, card } if *known == source && *card == target)).ok_or("target tap must be represented by a resolving-effect PermanentTapped receipt")?;
    let ability_resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability: ABILITY_ID, .. } if *known == source)).ok_or("Sandsower AbilityResolved receipt missing")?;
    let attack = event_index_after(&capture.events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|attacker| *attacker != source)) ).ok_or("pilot did not resume combat after the Sandsower activation")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } if *amount > 0)).ok_or("pilot dealt no combat damage after tapping the Watchwolf")?;
    let ended = unique_index(&capture.events, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal P0 GameEnded receipt missing")?;
    if tap_indices.len() != 3 || tap_indices.iter().any(|index| *index >= activated) || !(paid < cast && cast < resolved && resolved < entered && wolf_cast < wolf_resolved && wolf_resolved < wolf_entered && entered < activated && activated < target_tap && target_tap < ability_resolved && ability_resolved < attack && attack < damage && damage < ended) {
        return Err(format!("Sandsower cast, cost, resolution, and terminal combat event order differs: cast=({paid},{cast},{resolved},{entered}), opponent=({wolf_cast},{wolf_resolved},{wolf_entered}), activation=({tap_indices:?},{activated},{target_tap},{ability_resolved}), combat=({attack},{damage},{ended})"));
    }
    let selected: BTreeSet<_> = [paid, cast, resolved, entered, wolf_cast, wolf_resolved, wolf_entered]
        .into_iter().chain(tap_indices.iter().copied()).chain([activated, target_tap, ability_resolved, attack, damage, ended]).collect();
    let projected = project_events(&capture.events);
    let events: Vec<_> = selected.iter().map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string())).collect::<Result<_, _>>()?;
    Ok(json!({
        "id": "rav-sandsower-three-creature-tap-target-full-game",
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": capture.rejected,
        "sandsower": {"object": source.0, "collector_number": 28, "mana_cost": "{3}{W}", "mana_paid": capture.spell_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": capture.power, "toughness": capture.toughness, "resolved": capture.spell_resolved},
        "activation": {"source_object": source.0, "ability_id": ABILITY_ID, "mana_cost": "{0}", "source_tap_cost": false, "source_tapped_after_activation": capture.source_tapped_after_activation, "active_player_at_activation": capture.active_player_at_activation, "target_object": target.0, "target_definition": capture.target_definition, "target_controller": capture.target_controller, "target_tapped_before": capture.target_tapped_before, "target_tapped_after_activation": capture.target_tapped_after_activation, "target_tapped_after_resolution": capture.target_tapped_after_resolution, "target_tap_event_count": capture.target_tap_events, "additional_tap_payers": capture.payers.iter().map(|payer| json!({"object": payer.object.0, "definition": payer.definition, "tapped_after_activation": payer.tapped_after_activation})).collect::<Vec<_>>()},
        "combat": {"opponent_watchwolf_object": wolf.0, "attack_turns": capture.attack_turns, "damage_to_opponent": capture.damage_to_opponent, "life_at_terminal": capture.life},
        "ordered_event_sequence": selected.iter().copied().collect::<Vec<_>>(),
        "events": events,
        "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
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
    let definition_id = match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), CardSemanticStatus::CatalogOnly { .. } => None };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
}

fn candidate_kind() -> &'static str { if std::env::args().any(|argument| argument == "--candidate") { "agent" } else { "reference" } }

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() { if argument == "--output-root" { return args.next().map(std::path::PathBuf::from); } }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
