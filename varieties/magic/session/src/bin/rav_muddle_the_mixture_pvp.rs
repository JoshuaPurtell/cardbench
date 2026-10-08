//! Full-game PvP authority for RAV Muddle the Mixture's counterspell and Transmute.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionVisibility, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, LibrarySearchDestination, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
use syn::{Expr, Item, ReturnType, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-MUDDLE-THE-MIXTURE";
const CARD_NAME: &str = "Muddle the Mixture";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const RAIN_ID: &str = "RAV-RAIN-OF-EMBERS";
const ISLAND_ID: &str = "RAV-ISLAND";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const MOUNTAIN_ID: &str = "RAV-MOUNTAIN";
const TRANSMUTE_ID: &str = "transmute";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/muddle_the_mixture.rs";
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned definition for RAV Muddle the Mixture.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed instant, counterspell effect, and hand-zone Transmute cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MUDDLE-THE-MIXTURE",
        name: "Muddle the Mixture",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-target-instant-or-sorcery-spell",
            "transmute",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
    }
}
"#;
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 48;
const MAX_MOVES: u32 = 8_000;
const OPPONENT_SPELL_TURN: u32 = 12;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/muddle_the_mixture.rs"]
mod candidate_muddle_the_mixture;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Muddle the Mixture result: {error}");
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
            eprintln!("Muddle the Mixture PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongSpellCost,
    WrongCounterEffect,
    BroadCounterTarget,
    NoTransmute,
    WrongTransmuteCost,
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
    transmute_selection: Option<ObjectId>,
    transmute_found: Option<ObjectId>,
    transmute_found_definition: Option<&'static str>,
    transmute_found_in_hand: bool,
    transmute_resolved: bool,
    transmute_search_private: bool,
    transmute_hidden_from_opponent: bool,
    transmute_decision: Option<u64>,
    transmute_source_zone: Option<Zone>,
    transmute_activation_turn: Option<u32>,
    transmute_activation_step: Option<Step>,
    transmute_activation_actor: Option<PlayerId>,
    transmute_sorcery_timing_valid: bool,
    counter_source: Option<ObjectId>,
    counter_spell: Option<ObjectId>,
    counter_spell_definition: Option<&'static str>,
    counter_spell_owner: Option<PlayerId>,
    counter_spell_cost: Option<ManaCost>,
    counter_spell_is_sorcery: bool,
    counter_spell_payment: Vec<Color>,
    counter_source_payment: Vec<Color>,
    counter_source_resolved: bool,
    counter_source_zone: Option<Zone>,
    countered: bool,
    counter_target_zone: Option<Zone>,
    target_spell_resolved: bool,
    pilot_watchwolf: Option<ObjectId>,
    pilot_watchwolf_owner: Option<PlayerId>,
    pilot_watchwolf_controller: Option<PlayerId>,
    attack_turns: Vec<u32>,
    damage_to_opponent: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Muddle the Mixture full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_spell_cost_mutation_detected": mutation_detected(seed, Mutation::WrongSpellCost),
        "wrong_counter_effect_mutation_detected": mutation_detected(seed, Mutation::WrongCounterEffect),
        "broad_counter_target_mutation_detected": mutation_detected(seed, Mutation::BroadCounterTarget),
        "transmute_removed_mutation_detected": mutation_detected(seed, Mutation::NoTransmute),
        "wrong_transmute_cost_mutation_detected": mutation_detected(seed, Mutation::WrongTransmuteCost),
        "compile_valid_injected_side_effect_candidate_rejected": source_validation["compile_valid_injected_side_effect_candidate_rejected"],
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_spell_cost_mutation_detected",
        "wrong_counter_effect_mutation_detected",
        "broad_counter_target_mutation_detected",
        "transmute_removed_mutation_detected",
        "wrong_transmute_cost_mutation_detected",
        "compile_valid_injected_side_effect_candidate_rejected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Muddle the Mixture gameplay mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 60, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting and targeting the instant",
                "117: priority and instant-speed responses",
                "608.2b-608.3: target legality and spell resolution",
                "701.5: countering a spell",
                "702.53: Transmute",
                "701.19: library search, reveal, and shuffle",
                "510: combat damage and terminal game state"
            ],
            "source_contract": "RAV Muddle the Mixture #60 is a {U}{U} instant that counters target instant or sorcery spell and has Transmute {1}{U}{U}.",
            "oracle_reconciliation": "The RAV printing uses converted mana cost in Transmute reminder text; current Oracle uses mana value. Both refer to the same printed-cost comparison for this card, and countering an instant or sorcery plus Transmute {1}{U}{U} are unchanged.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "candidate_source_validation": source_validation,
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
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

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_muddle_the_mixture::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongSpellCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Blue]);
        }
        Mutation::WrongCounterEffect => {
            definition.effects = vec![Effect::CounterTargetSpellToOwnersHand];
        }
        Mutation::BroadCounterTarget => {
            definition.effects = vec![Effect::CounterTargetSpell];
        }
        Mutation::NoTransmute => {
            definition
                .keywords
                .retain(|keyword| !matches!(keyword, Keyword::Transmute(_)));
        }
        Mutation::WrongTransmuteCost => {
            definition.keywords = vec![Keyword::Transmute(ManaCost::with_colors(
                2,
                [Color::Blue, Color::Blue],
            ))];
        }
    }
    definition
}

fn validate_candidate() -> Result<(), String> {
    let expected = expected_definition();
    let candidate = candidate_muddle_the_mixture::definition();
    if candidate != expected {
        return Err(
            "Muddle the Mixture candidate definition differs from its printed source contract"
                .to_owned(),
        );
    }
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Muddle the Mixture is missing from the RAV definition registry")?;
    if registered != candidate {
        return Err("Muddle the Mixture candidate definition is not uniquely represented in the RAV registry".to_owned());
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-target-instant-or-sorcery-spell",
            "transmute",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
    }
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
        .ok_or("Muddle the Mixture definition is missing from RAV registry")? =
        definition_for(mutation);
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())
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
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "trace_status": if definition_id == Some(CARD_ID) { "passed" } else { "not_run" },
        "credited": false,
        "semantic_status": format!("{:?}", card.semantic_status)
    })
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_MUDDLE_THE_MIXTURE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_MUDDLE_THE_MIXTURE_SEED must be an unsigned integer")?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Muddle the Mixture full-game trace satisfied the contract within seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        (ISLAND_ID, 40),
        (FOREST_ID, 6),
        (PLAINS_ID, 6),
    ]);
    let opponent = deck(&[
        (RAIN_ID, 4),
        (WATCHWOLF_ID, 4),
        (MOUNTAIN_ID, 18),
        (FOREST_ID, 17),
        (PLAINS_ID, 17),
    ]);
    for (name, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!(
                "Muddle the Mixture {name} fixture is not a legal 60-card deck"
            ));
        }
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
        Box::new(MuddlePolicy::new(PlayerId(0), mutation)),
        Box::new(MuddlePolicy::new(PlayerId(1), mutation)),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        transmute_source: None,
        transmute_source_incarnation: None,
        transmute_resolved_incarnation: None,
        transmute_cost: None,
        transmute_selection: None,
        transmute_found: None,
        transmute_found_definition: None,
        transmute_found_in_hand: false,
        transmute_resolved: false,
        transmute_search_private: false,
        transmute_hidden_from_opponent: true,
        transmute_decision: None,
        transmute_source_zone: None,
        transmute_activation_turn: None,
        transmute_activation_step: None,
        transmute_activation_actor: None,
        transmute_sorcery_timing_valid: false,
        counter_source: None,
        counter_spell: None,
        counter_spell_definition: None,
        counter_spell_owner: None,
        counter_spell_cost: None,
        counter_spell_is_sorcery: false,
        counter_spell_payment: vec![],
        counter_source_payment: vec![],
        counter_source_resolved: false,
        counter_source_zone: None,
        countered: false,
        counter_target_zone: None,
        target_spell_resolved: false,
        pilot_watchwolf: None,
        pilot_watchwolf_owner: None,
        pilot_watchwolf_controller: None,
        attack_turns: vec![],
        damage_to_opponent: 0,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!(
                "Muddle the Mixture match exceeded its action bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Muddle the Mixture policy seat is absent")?;
        let action = if let Some(choice) = &view.library_search_choice {
            if player == PlayerId(0) {
                capture.transmute_search_private = true;
                capture.transmute_decision = Some(choice.decision.0);
                let opponent_view = game
                    .view_for_player(PlayerId(1))
                    .map_err(|error| error.to_string())?;
                capture.transmute_hidden_from_opponent &=
                    opponent_view.library_search_choice.is_none();
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
        match &action {
            PolicyAction::Transmute { card } if player == PlayerId(0) => {
                if capture.transmute_source.is_none() {
                    capture.transmute_source = Some(*card);
                    capture.transmute_source_incarnation =
                        game.object(*card).ok().map(|object| object.incarnation);
                    capture.transmute_activation_turn = Some(view.turn);
                    capture.transmute_activation_step = Some(view.step);
                    capture.transmute_activation_actor = Some(player);
                    capture.transmute_sorcery_timing_valid = view.active_player == player
                        && view.priority == player
                        && view.step == Step::PrecombatMain
                        && view.stack_depth == 0;
                }
            }
            PolicyAction::ChooseLibrarySearchCard {
                source, selected, ..
            } if Some(*source) == capture.transmute_source => {
                capture.transmute_selection = *selected;
            }
            PolicyAction::CastWithPayment { request, .. }
                if player == PlayerId(0)
                    && game
                        .card_definition(request.card)
                        .is_ok_and(|definition| definition.id == CARD_ID)
                    && capture.counter_source.is_none() =>
            {
                if let Some(Target::Spell(target)) = request.targets.first() {
                    capture.counter_source = Some(request.card);
                    capture.counter_spell = Some(*target);
                    capture.counter_spell_definition = game
                        .card_definition(*target)
                        .ok()
                        .map(|definition| definition.id);
                    if let Ok(definition) = game.card_definition(*target) {
                        capture.counter_spell_cost = Some(definition.mana_cost.clone());
                        capture.counter_spell_is_sorcery =
                            definition.card_types.contains(&CardType::Sorcery);
                    }
                    capture.counter_spell_owner =
                        game.object(*target).ok().map(|object| object.owner);
                }
            }
            _ => {}
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Muddle the Mixture move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: TRANSMUTE_ID,
                mana_cost,
            } if Some(*source) == capture.transmute_source => {
                capture.transmute_cost = Some(mana_cost.clone());
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                definition,
                ability: TRANSMUTE_ID,
            } if Some(*source) == capture.transmute_source && *definition == CARD_ID => {
                capture.transmute_source_incarnation = Some(*source_incarnation);
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability: TRANSMUTE_ID,
            } if Some(*source) == capture.transmute_source => {
                capture.transmute_resolved = true;
                capture.transmute_resolved_incarnation = Some(*source_incarnation);
            }
            GameEvent::DiscardedAsAbilityCost {
                player: PlayerId(0),
                source,
                card,
            } if Some(*source) == capture.transmute_source && source == card => {}
            GameEvent::DecisionOpened {
                decision,
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                ..
            } if capture.transmute_decision == Some(decision.0) => {}
            GameEvent::DecisionOpened {
                decision,
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                ..
            } => {
                capture.transmute_decision.get_or_insert(decision.0);
            }
            GameEvent::DecisionCompleted {
                decision,
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
            } if capture.transmute_decision == Some(decision.0) => {
                capture.transmute_search_private = true;
            }
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition,
            } if Some(*card) == capture.transmute_selection => {
                capture.transmute_found_definition = Some(definition);
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if Some(*card) == capture.transmute_selection => {
                capture.transmute_found = Some(*card);
                capture.transmute_found_in_hand = true;
            }
            GameEvent::LibrarySearchResolved {
                player: PlayerId(0),
                source,
                found,
                destination: LibrarySearchDestination::Hand,
            } if Some(*source) == capture.transmute_source => {
                capture.transmute_found = *found;
            }
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found,
            } if Some(*discarded) == capture.transmute_source => {
                capture.transmute_found = *found;
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            } if Some(*card) == capture.counter_spell => {
                capture.counter_spell_payment = colors.clone();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == capture.counter_source => {
                capture.counter_source_payment = colors.clone();
            }
            GameEvent::SpellCountered { card, source }
                if Some(*source) == capture.counter_source
                    && Some(*card) == capture.counter_spell =>
            {
                capture.countered = true;
            }
            GameEvent::SpellResolved { card } if Some(*card) == capture.counter_source => {
                capture.counter_source_resolved = true;
            }
            GameEvent::SpellResolved { card } if Some(*card) == capture.counter_spell => {
                capture.target_spell_resolved = true;
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == capture.counter_source => {
                capture.counter_source_zone = Some(Zone::Graveyard);
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == capture.counter_spell => {
                capture.counter_target_zone = Some(Zone::Graveyard);
            }
            GameEvent::CardMoved { card, to }
                if Some(*card) == capture.counter_spell && *to != Zone::Graveyard =>
            {
                capture.counter_target_zone = Some(*to);
            }
            GameEvent::CardMoved { card, to } if Some(*card) == capture.transmute_source => {
                capture.transmute_source_zone = Some(*to);
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if game
                .object(*card)
                .is_ok_and(|object| object.owner == PlayerId(0))
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
            {
                let object = game.object(*card).map_err(|error| error.to_string())?;
                capture.pilot_watchwolf = Some(*card);
                capture.pilot_watchwolf_owner = Some(object.owner);
                capture.pilot_watchwolf_controller = Some(object.controller);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if capture
                .pilot_watchwolf
                .is_some_and(|watchwolf| attackers.contains(&watchwolf)) =>
            {
                if !capture.attack_turns.contains(&game.turn) {
                    capture.attack_turns.push(game.turn);
                }
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == capture.pilot_watchwolf => {
                capture.damage_to_opponent += *amount;
            }
            _ => {}
        }
    }
    Ok(())
}

struct MuddlePolicy {
    player: PlayerId,
    mutation: Mutation,
    transmute_started: bool,
    found_muddle: Option<ObjectId>,
    counter_cast: bool,
    opponent_spell_cast: bool,
    watchwolf_cast: bool,
}

impl MuddlePolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            transmute_started: false,
            found_muddle: None,
            counter_cast: false,
            opponent_spell_cast: false,
            watchwolf_cast: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = &mut available.iter_mut().find(|(known, _)| known == color)?.1;
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
        let mut missing_color = None;
        for color in &cost.colored {
            let required = cost.colored.iter().filter(|known| *known == color).count() as u8;
            if view.mana_pool.amount(*color) < required {
                missing_color = Some(*color);
                break;
            }
        }
        if missing_color.is_none()
            && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
        {
            missing_color = Some(Color::Blue);
        }
        let needed = missing_color?;
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&needed)
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = if land.mana_colors.contains(&needed) {
            needed
        } else {
            *land.mana_colors.iter().next()?
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let has = |definition: &str| {
            view.own_battlefield.iter().any(|card| {
                card.card_types.contains(&CardType::Land) && card.definition == Some(definition)
            })
        };
        let wanted = if self.player == PlayerId(0) {
            if view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Blue)
                })
                .count()
                < 5
            {
                ISLAND_ID
            } else if !has(FOREST_ID) {
                FOREST_ID
            } else if !has(PLAINS_ID) {
                PLAINS_ID
            } else {
                ISLAND_ID
            }
        } else if self.mutation == Mutation::BroadCounterTarget {
            if !has(FOREST_ID) {
                FOREST_ID
            } else {
                PLAINS_ID
            }
        } else {
            MOUNTAIN_ID
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(wanted))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast_counter(&mut self, view: &GameView, target: ObjectId) -> Option<PolicyAction> {
        let card = self
            .found_muddle
            .and_then(|found| view.hand.iter().find(|card| card.id == found))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            })?;
        let cost = definition_for(self.mutation).mana_cost;
        if let Some(mana_selection) = Self::payment(view, &cost) {
            self.counter_cast = true;
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![Target::Spell(target)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, &cost)
        }
    }

    fn cast_watchwolf(&mut self, view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))?;
        let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        if let Some(mana_selection) = Self::payment(view, &cost) {
            self.watchwolf_cast = true;
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, &cost)
        }
    }

    fn cast_opponent_spell(&mut self, view: &GameView) -> Option<PolicyAction> {
        let definition = if self.mutation == Mutation::BroadCounterTarget {
            WATCHWOLF_ID
        } else {
            RAIN_ID
        };
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        let cost = if definition == WATCHWOLF_ID {
            ManaCost::with_colors(0, [Color::Green, Color::White])
        } else {
            ManaCost::with_colors(1, [Color::Red])
        };
        if let Some(mana_selection) = Self::payment(view, &cost) {
            self.opponent_spell_cast = true;
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, &cost)
        }
    }
}

impl CodePolicy for MuddlePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.muddle-counter-transmute-combat.v1"
        } else {
            "rav.rain-of-embers-single-spell.v1"
        }
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .library_search_choice
            .as_ref()
            .expect("Muddle Transmute choice is visible to its controller");
        let selected = choice
            .cards
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
            .map(|card| card.id);
        self.found_muddle = selected;
        PolicyAction::ChooseLibrarySearchCard {
            decision: choice.decision,
            source: choice.source,
            selected,
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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

        let target_definition = if self.mutation == Mutation::BroadCounterTarget {
            WATCHWOLF_ID
        } else {
            RAIN_ID
        };
        if self.player == PlayerId(0)
            && !self.counter_cast
            && let Some(target) = view
                .stack_spells
                .iter()
                .find(|spell| spell.definition == Some(target_definition))
                .map(|spell| spell.id)
            && let Some(action) = self.cast_counter(view, target)
        {
            return action;
        }

        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && view.step == Step::PrecombatMain {
            if let Some(action) = self.play_land(view) {
                return action;
            }
            if self.player == PlayerId(0) {
                let definition = definition_for(self.mutation);
                if !self.transmute_started
                    && let Some(cost) = definition.transmute_cost()
                    && let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                {
                    if let Some(_payment) = Self::payment(view, cost) {
                        self.transmute_started = true;
                        return PolicyAction::Transmute { card: card.id };
                    }
                    if let Some(action) = Self::make_mana(view, cost) {
                        return action;
                    }
                }
                if !self.watchwolf_cast
                    && let Some(action) = self.cast_watchwolf(view)
                {
                    return action;
                }
            } else if !self.opponent_spell_cast
                && view.turn >= OPPONENT_SPELL_TURN
                && let Some(action) = self.cast_opponent_spell(view)
            {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    let transmute_source = capture
        .transmute_source
        .ok_or("Muddle Transmute was not activated")?;
    let found = capture
        .transmute_found
        .ok_or("Muddle Transmute did not find a card")?;
    let counter_source = capture
        .counter_source
        .ok_or("Muddle was not cast to counter a spell")?;
    let target = capture
        .counter_spell
        .ok_or("Muddle has no exact spell target")?;
    let expected_transmute = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
    if capture.winner != Some(0)
        || capture.rejected != 0
        || transmute_source == found
        || counter_source != found
        || capture.transmute_selection != Some(found)
        || capture.transmute_found_definition != Some(CARD_ID)
        || !capture.transmute_found_in_hand
        || !capture.transmute_resolved
        || capture.transmute_source_incarnation.is_none()
        || capture.transmute_source_incarnation != capture.transmute_resolved_incarnation
        || capture.transmute_cost.as_ref() != Some(&expected_transmute)
        || capture.transmute_source_zone != Some(Zone::Graveyard)
        || capture.transmute_activation_actor != Some(PlayerId(0))
        || capture.transmute_activation_step != Some(Step::PrecombatMain)
        || capture.transmute_activation_turn.is_none()
        || !capture.transmute_sorcery_timing_valid
        || !capture.transmute_search_private
        || !capture.transmute_hidden_from_opponent
        || capture.counter_spell_definition != Some(RAIN_ID)
        || capture.counter_spell_cost.as_ref() != Some(&ManaCost::with_colors(1, [Color::Red]))
        || !capture.counter_spell_is_sorcery
        || capture.counter_spell_owner != Some(PlayerId(1))
        || capture.counter_spell_payment != [Color::Red, Color::Red]
        || capture.counter_source_payment != [Color::Blue, Color::Blue]
        || !capture.countered
        || capture.counter_target_zone != Some(Zone::Graveyard)
        || !capture.counter_source_resolved
        || capture.counter_source_zone != Some(Zone::Graveyard)
        || capture.target_spell_resolved
        || capture.pilot_watchwolf_owner != Some(PlayerId(0))
        || capture.pilot_watchwolf_controller != Some(PlayerId(0))
        || capture.attack_turns.len() < 7
        || capture.damage_to_opponent < 20
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err(format!(
            "Muddle trace mismatch: winner={:?}, transmute={transmute_source:?}->{found:?} cost={:?} resolved={} private={} hidden={}, counter={counter_source:?}->{target:?} def={:?} pay={:?}/{:?} countered={} zones={:?}/{:?} attacks={} damage={} life={:?}",
            capture.winner,
            capture.transmute_cost,
            capture.transmute_resolved,
            capture.transmute_search_private,
            capture.transmute_hidden_from_opponent,
            capture.counter_spell_definition,
            capture.counter_spell_payment,
            capture.counter_source_payment,
            capture.countered,
            capture.counter_target_zone,
            capture.counter_source_zone,
            capture.attack_turns.len(),
            capture.damage_to_opponent,
            capture.life
        ));
    }
    if capture.counter_source == Some(transmute_source) || capture.counter_source_payment.is_empty()
    {
        return Err(
            "Muddle counter branch differs from the printed instant/sorcery target contract"
                .to_owned(),
        );
    }

    let events = &capture.events;
    let index = |name: &str, pred: &dyn Fn(&GameEvent) -> bool| {
        unique_index(events, pred).ok_or_else(|| format!("Muddle trace is missing {name}"))
    };
    let transmute_paid = index(
        "Transmute AbilityManaPaid",
        &|event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: TRANSMUTE_ID, mana_cost } if *source == transmute_source && *mana_cost == expected_transmute),
    )?;
    let transmute_discarded = index(
        "Transmute discard receipt",
        &|event| matches!(event, GameEvent::DiscardedAsAbilityCost { player: PlayerId(0), source, card } if *source == transmute_source && *card == transmute_source),
    )?;
    let transmute_source_moved = index(
        "Transmute source graveyard move",
        &|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == transmute_source),
    )?;
    let transmute_activated = index(
        "Transmute AbilityActivated",
        &|event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: TRANSMUTE_ID, .. } if *source == transmute_source),
    )?;
    let decision = capture
        .transmute_decision
        .ok_or("Muddle private Transmute decision was not recorded")?;
    let decision_opened = index(
        "private library-search decision",
        &|event| matches!(event, GameEvent::DecisionOpened { decision: id, player: PlayerId(0), kind: DecisionKind::LibrarySearch, visibility: DecisionVisibility::Private, .. } if id.0 == decision),
    )?;
    let decision_completed = index(
        "library-search decision completion",
        &|event| matches!(event, GameEvent::DecisionCompleted { decision: id, player: PlayerId(0), kind: DecisionKind::LibrarySearch } if id.0 == decision),
    )?;
    let found_revealed = index(
        "revealed Transmute selection",
        &|event| matches!(event, GameEvent::CardRevealed { player: PlayerId(0), card, definition: CARD_ID } if *card == found),
    )?;
    let found_moved = index(
        "selected Muddle hand move",
        &|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == found),
    )?;
    let search_resolved = index(
        "exact Transmute search resolution",
        &|event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source, found: Some(selected), destination: LibrarySearchDestination::Hand } if *source == transmute_source && *selected == found),
    )?;
    let library_shuffle = events
        .iter()
        .enumerate()
        .skip(search_resolved + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::LibraryShuffled {
                    player: PlayerId(0),
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("Muddle Transmute did not shuffle its controller's library after searching")?;
    let transmuted = index(
        "Transmuted receipt",
        &|event| matches!(event, GameEvent::Transmuted { player: PlayerId(0), discarded, found: Some(selected) } if *discarded == transmute_source && *selected == found),
    )?;
    let transmute_resolved = index(
        "Transmute AbilityResolved",
        &|event| matches!(event, GameEvent::AbilityResolved { source, ability: TRANSMUTE_ID, .. } if *source == transmute_source),
    )?;
    let target_cast = index(
        "opponent sorcery SpellCast",
        &|event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target),
    )?;
    let counter_paid = index(
        "Muddle SpellManaPaid",
        &|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == counter_source && colors == &[Color::Blue, Color::Blue]),
    )?;
    let counter_cast = index(
        "Muddle SpellCast",
        &|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == counter_source),
    )?;
    let spell_countered = index(
        "SpellCountered",
        &|event| matches!(event, GameEvent::SpellCountered { card, source } if *card == target && *source == counter_source),
    )?;
    let target_graveyard = index(
        "countered sorcery graveyard move",
        &|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target),
    )?;
    let counter_resolved = index(
        "Muddle SpellResolved",
        &|event| matches!(event, GameEvent::SpellResolved { card } if *card == counter_source),
    )?;
    let counter_graveyard = index(
        "Muddle spell graveyard move",
        &|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == counter_source),
    )?;
    let game_ended = index("terminal GameEnded", &|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })?;
    let ordered = vec![
        transmute_paid,
        transmute_discarded,
        transmute_source_moved,
        transmute_activated,
        decision_opened,
        decision_completed,
        found_revealed,
        found_moved,
        search_resolved,
        library_shuffle,
        transmuted,
        transmute_resolved,
        target_cast,
        counter_paid,
        counter_cast,
        spell_countered,
        target_graveyard,
        counter_resolved,
        counter_graveyard,
        game_ended,
    ];
    if ordered.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            "Muddle search, counterspell, and terminal receipts are out of order".to_owned(),
        );
    }
    let projected = project_events(events);
    Ok(json!({
        "trace_id": "rav-muddle-the-mixture-transmute-counter-full-game",
        "passed": true,
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": capture.rejected,
        "transmute": {
            "source": transmute_source.0,
            "source_definition": CARD_ID,
            "source_incarnation": capture.transmute_source_incarnation,
            "mana_cost": "{1}{U}{U}",
            "activation_turn": capture.transmute_activation_turn,
            "activation_step": format!("{:?}", capture.transmute_activation_step),
            "activation_actor": capture.transmute_activation_actor.map(|player| player.0),
            "sorcery_speed_timing": capture.transmute_sorcery_timing_valid,
            "found": found.0,
            "found_definition": capture.transmute_found_definition,
            "revealed": true,
            "destination": "Hand",
            "found_in_hand": capture.transmute_found_in_hand,
            "source_zone_after_cost": format!("{:?}", capture.transmute_source_zone),
            "private_decision": capture.transmute_search_private,
            "hidden_from_opponent_during_choice": capture.transmute_hidden_from_opponent,
            "shuffle_after_search": true,
            "resolved": capture.transmute_resolved
        },
        "counter": {
            "source": counter_source.0,
            "source_definition": CARD_ID,
            "mana_cost": "{U}{U}",
            "mana_paid": capture.counter_source_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "target_spell": target.0,
            "target_definition": capture.counter_spell_definition,
            "target_card_type": if capture.counter_spell_is_sorcery { "Sorcery" } else { "Other" },
            "target_mana_cost": "{1}{R}",
            "target_owner": capture.counter_spell_owner.map(|player| player.0),
            "target_mana_paid": capture.counter_spell_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "countered": capture.countered,
            "target_zone_after": format!("{:?}", capture.counter_target_zone),
            "source_resolved": capture.counter_source_resolved,
            "source_zone_after": format!("{:?}", capture.counter_source_zone),
            "target_spell_resolved": capture.target_spell_resolved
        },
        "combat": {
            "attacking_watchwolf": capture.pilot_watchwolf.map(|card| card.0),
            "watchwolf_owner": capture.pilot_watchwolf_owner.map(|player| player.0),
            "watchwolf_controller": capture.pilot_watchwolf_controller.map(|player| player.0),
            "attack_turns": capture.attack_turns,
            "damage_to_opponent": capture.damage_to_opponent,
            "life_at_terminal": capture.life
        },
        "ordered_event_indices": ordered,
        "events": projected
    }))
}

fn unique_index(events: &[GameEvent], predicate: &dyn Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter(|(_, event)| predicate(event))
        .map(|(index, _)| index);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Muddle the Mixture source: {error}"))?;
    validate_candidate_source_text(&source)?;

    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = source.replacen(
        signature,
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-muddle-the-mixture-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Muddle side-effect mutation could not be constructed".to_owned(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Muddle source guard accepted an injected side effect".to_owned());
    }
    Ok(json!({
        "normalized_literal_ast_schema_checked_before_candidate_calls": true,
        "compile_valid_injected_side_effect_fixture_compiled_by_construction": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "guard_ran_before_candidate_function_calls": true,
        "rejection_stage": "pinned-source-and-declarative-AST-before-candidate-function-invocation"
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Muddle the Mixture candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err("Muddle source differs from the pinned literal source contract".to_owned());
    }

    let mut imports = BTreeSet::new();
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports.extend(flattened_use_paths(item).ok_or("unsupported Muddle import")?);
            }
            Item::Fn(function)
                if function.sig.ident == "definition"
                    && matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none()
                    && function.sig.variadic.is_none()
                    && matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("CardDefinition")))
                    && function.block.stmts.len() == 1
                    && function.attrs.iter().all(|attribute| {
                        (attribute.path().is_ident("doc")
                            && matches!(attribute.meta, syn::Meta::NameValue(_)))
                            || (attribute.path().is_ident("must_use")
                                && matches!(attribute.meta, syn::Meta::Path(_)))
                    }) =>
            {
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Muddle factory must directly return a CardDefinition literal".to_owned(),
                    );
                };
                let fields: BTreeSet<String> = value
                    .fields
                    .iter()
                    .filter_map(|field| match &field.member {
                        syn::Member::Named(name) => Some(name.to_string()),
                        _ => None,
                    })
                    .collect();
                let expected_fields = BTreeSet::from([
                    "id",
                    "name",
                    "set_code",
                    "mana_cost",
                    "colors",
                    "mana_colors",
                    "card_types",
                    "is_basic_land",
                    "supported_rules",
                    "power",
                    "toughness",
                    "keywords",
                    "effects",
                ]);
                if !path_is_type(&value.path, "CardDefinition")
                    || value.rest.is_some()
                    || fields.len() != value.fields.len()
                    || fields.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields
                    || !declarative_expr(&Expr::Struct(value.clone()))
                {
                    return Err(
                        "Muddle CardDefinition literal differs from its pinned schema".to_owned(),
                    );
                }
                definitions += 1;
            }
            _ => {
                return Err(
                    "Muddle module permits only imports and one literal definition function"
                        .to_owned(),
                );
            }
        }
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let expected =
        syn::parse_file(EXPECTED_CANDIDATE_SOURCE).expect("pinned Muddle candidate source parses");
    if definitions != 1
        || imports != expected_imports
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err(
            "Muddle source AST differs from the pinned RAV literal source contract".to_owned(),
        );
    }
    Ok(())
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn declarative_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            matches!(
                value
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .as_deref(),
                Some("CardDefinition")
            ) && value.attrs.is_empty()
                && value.rest.is_none()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let segments: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(segments.as_slice(), [owner, method] if (owner == "BTreeSet" && (method == "from" || method == "new")) || (owner == "ManaCost" && (method == "new" || method == "with_colors")) || (owner == "Keyword" && method == "Transmute"))
                        || matches!(segments.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            paths.push(next);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
