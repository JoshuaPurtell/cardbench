//! Full-game RAV Remand diagnostic authority.
//!
//! A shuffled constructed duel casts an opposing Watchwolf, responds with Remand,
//! observes the exact spell return to its owner's hand and Remand draw for its
//! controller, then continues to a terminal seat-zero combat win.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, PathArguments, ReturnType, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-REMAND";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const PILOT_THREAT_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/remand.rs";
const DECK_P0: &str = "rav_remand_blue_green_white_60";
const DECK_P1: &str = "rav_remand_watchwolf_green_white_60";
const MAX_TRACE_SEED: u64 = 96;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/remand.rs"]
mod candidate_remand;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Remand PvP result: {error}");
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
            eprintln!("Remand PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let candidate = candidate_remand::definition();
    if !definition_identity_valid(&candidate) {
        return Err("candidate Remand printed identity or characteristics differ".to_owned());
    }
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Remand is absent from the RAV card registry")?;
    if candidate != registered {
        return Err("RAV registry does not delegate Remand to the candidate module".to_owned());
    }

    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        ));
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match_with(candidate, seed)?;
    if !same_match(&captured, &replay) {
        return Err("Remand full-game replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "wrong_destination_mutation_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::WrongDestination)?,
        "missing_draw_mutation_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::MissingDraw)?,
        "missing_counter_mutation_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::MissingCounter)?,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["wrong_destination_mutation_detected_in_game"] != true
        || mutation_sensitivity["missing_draw_mutation_detected_in_game"] != true
        || mutation_sensitivity["missing_counter_mutation_detected_in_game"] != true
    {
        return Err("a Remand gameplay mutation escaped the full-game trace oracle".to_owned());
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 63, "printings": 1, "definition_id": CARD_ID, "name": "Remand"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority and instant-speed response windows",
                "115 Targets",
                "601.2 Casting spells and selecting targets",
                "608.2b-608.3 Target legality and resolution",
                "701.5 Countering spells",
                "121.2a Drawing cards",
                "510 Combat damage and terminal game state"
            ],
            "source_contract": "RAV Remand #63, {1}{U} instant: counter target spell; return it to its owner's hand, then draw a card",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": if candidate_kind() == "candidate" {
            "candidate_pilot_pass; independent reviewer approval remains"
        } else {
            "reference_pilot_pass; independent review and candidate trial remain"
        },
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "definition_id": definition_id,
        "name": card.name,
        "engine_status": engine_status,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false
    })
}

#[derive(Clone, Copy)]
enum Mutation {
    WrongDestination,
    MissingDraw,
    MissingCounter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    remand: Option<ObjectId>,
    remand_target: Option<ObjectId>,
    target: Option<ObjectId>,
    drawn: Option<ObjectId>,
    pilot_threats: Vec<ObjectId>,
    remand_payment: Vec<Color>,
    target_payment: Vec<Color>,
    zones: [Option<Zone>; 3],
    owners: [Option<PlayerId>; 3],
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_REMAND_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_REMAND_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        if !opening_hand_has_remand(seed)? {
            continue;
        }
        match run_match(seed).and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced the full-game Remand trace: {last_error}"
    ))
}

fn opening_hand_has_remand(seed: u64) -> Result<bool, String> {
    let (pilot, opponent) = task_decks();
    let mut game = Game::new(card_definitions(), 2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let pilot_view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    Ok(pilot_view
        .hand
        .iter()
        .any(|card| card.definition == Some(CARD_ID)))
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}

fn run_match(seed: u64) -> Result<Captured, String> {
    run_match_with(candidate_remand::definition(), seed)
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_remand::definition();
    match mutation {
        Mutation::WrongDestination => {
            definition.effects = vec![Effect::CounterTargetSpell, Effect::DrawController];
        }
        Mutation::MissingDraw => {
            definition.effects = vec![Effect::CounterTargetSpellToOwnersHand];
        }
        Mutation::MissingCounter => {
            definition.effects = vec![Effect::DrawController];
        }
    }
    definition
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let definition = mutated_definition(mutation);
    if !definition_identity_valid(&definition) {
        return Ok(false);
    }
    let captured = run_match_with(definition, seed)?;
    if captured.terminal != "Winner(PlayerId(0))" || captured.rejected != 0 {
        return Ok(false);
    }
    let diagnostic = validate_trace(&captured, seed).err().unwrap_or_default();
    let expected = match mutation {
        Mutation::WrongDestination => "Remand countered the target to the wrong destination",
        Mutation::MissingDraw => "Remand did not draw exactly one card",
        Mutation::MissingCounter => "Remand did not counter the exact target spell",
    };
    Ok(diagnostic.contains(expected))
}

fn run_match_with(definition: CardDefinition, seed: u64) -> Result<Captured, String> {
    if !definition_identity_valid(&definition) {
        return Err("Remand mutation changed printed identity or card frame".to_owned());
    }
    let (pilot, opponent) = task_decks();
    for (name, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!(
                "{name} has {size} cards; expected a legal 60-card main deck"
            ));
        }
    }

    let pilot_can_select_spell_target = definition.effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::CounterTargetSpellToOwnersHand
                | Effect::CounterTargetSpell
                | Effect::CounterTargetInstantOrSorcerySpell
                | Effect::CounterTargetNoncreatureSpell
        )
    });
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Remand is absent from the RAV registry")? = definition;
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut pilot_policy = RemandPolicy::new(PlayerId(0), pilot_can_select_spell_target);
    let mut target_policy = RemandPolicy::new(PlayerId(1), false);
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut drawn = None;
    let mut drawn_zone_snapshot = None;
    let mut drawn_owner_snapshot = None;
    let mut target_owner_snapshot = None;
    let mut target_zone_after_counter = None;
    let mut target_counter_seen = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(48) || accepted >= config.max_policy_moves {
            return Err("Remand match exceeded its turn or policy-move limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let event_log_start = game.event_log.len();
        let p0_hand_before: BTreeSet<_> = game
            .player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .hand
            .iter()
            .copied()
            .collect();
        let library_top_before = game
            .player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .library
            .last()
            .copied();
        let action = if player == PlayerId(0) {
            policy_action(&mut pilot_policy, &view)
        } else {
            policy_action(&mut target_policy, &view)
        };
        let policy_id = if player == PlayerId(0) {
            pilot_policy.id()
        } else {
            target_policy.id()
        };
        let action_debug = format!("{action:?}");
        attempted += 1;
        game.submit_policy_move(player, policy_id, action)
            .map_err(|error| {
                format!(
                    "Remand policy move rejected at turn {} step {:?} for {action_debug}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[event_log_start..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == TARGET_ID)
                    && target_owner_snapshot.is_none() =>
                {
                    target_owner_snapshot = game.object(*card).ok().map(|object| object.owner);
                }
                GameEvent::SpellCountered { card, source }
                    if pilot_policy.remand_target == Some(*card)
                        && game
                            .card_definition(*source)
                            .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    target_counter_seen = true;
                }
                GameEvent::CardMoved { card, to }
                    if target_counter_seen
                        && pilot_policy.remand_target == Some(*card)
                        && target_zone_after_counter.is_none() =>
                {
                    target_zone_after_counter = Some(*to);
                }
                _ => {}
            }
        }
        if drawn.is_none()
            && game.event_log[event_log_start..].iter().any(|event| matches!(event, GameEvent::SpellResolved { card } if game.card_definition(*card).is_ok_and(|card| card.id == CARD_ID)))
        {
            if let Some(top) = library_top_before
                && !p0_hand_before.contains(&top)
                && game.zone_of(top) == Some(Zone::Hand)
            {
                drawn = Some(top);
                drawn_zone_snapshot = game.zone_of(top);
                drawn_owner_snapshot = game.object(top).ok().map(|object| object.owner);
            }
        }
    }

    let events = game.event_log.clone();
    let remand = find_spell(&game, &events, PlayerId(0), CARD_ID);
    let target = pilot_policy
        .remand_target
        .or_else(|| find_spell(&game, &events, PlayerId(1), TARGET_ID));
    let pilot_threats = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == PILOT_THREAT_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let spell_payment = |seat: PlayerId, object: Option<ObjectId>| {
        object
            .and_then(|object| {
                events.iter().find_map(|event| match event {
                    GameEvent::SpellManaPaid {
                        player,
                        card,
                        colors,
                    } if *player == seat && *card == object => Some(colors.clone()),
                    _ => None,
                })
            })
            .unwrap_or_default()
    };
    let remand_payment = spell_payment(PlayerId(0), remand);
    let target_payment = spell_payment(PlayerId(1), target);
    let zones = [
        remand.and_then(|card| game.zone_of(card)),
        target_zone_after_counter.or_else(|| target.and_then(|card| game.zone_of(card))),
        drawn_zone_snapshot.or_else(|| drawn.and_then(|card| game.zone_of(card))),
    ];
    let owners = [
        remand.and_then(|card| game.object(card).ok().map(|object| object.owner)),
        target_owner_snapshot,
        drawn_owner_snapshot
            .or_else(|| drawn.and_then(|card| game.object(card).ok().map(|object| object.owner))),
    ];
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        remand,
        remand_target: pilot_policy.remand_target,
        target,
        drawn,
        pilot_threats,
        remand_payment,
        target_payment,
        zones,
        owners,
        life: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
    })
}

fn task_decks() -> (DeckList, DeckList) {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: PILOT_THREAT_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 29,
            },
        ],
        sideboard: vec![],
    };
    (pilot, opponent)
}

fn find_spell(game: &Game, events: &[GameEvent], seat: PlayerId, id: &str) -> Option<ObjectId> {
    events.iter().find_map(|event| match event {
        GameEvent::SpellCast { player, card }
            if *player == seat
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == id) =>
        {
            Some(*card)
        }
        _ => None,
    })
}

fn policy_action(policy: &mut RemandPolicy, view: &GameView) -> PolicyAction {
    if let Some(action) = policy.propose_pending_decision(view) {
        return action;
    }
    if view.optional_triggered_ability_choice.is_some() {
        return policy.propose_optional_triggered_ability(view);
    }
    if view.draw_replacement_pending {
        return policy.propose_draw_replacement(view);
    }
    if view.private_library_choice.is_some() {
        return policy.propose_private_library_choice(view);
    }
    if view.private_opponent_library_choice.is_some() {
        return policy.propose_private_opponent_library_choice(view);
    }
    if view.library_search_choice.is_some() {
        return policy.propose_library_search_choice(view);
    }
    policy.propose_move(view)
}

struct RemandPolicy {
    player: PlayerId,
    remand_can_select_spell_target: bool,
    target_cast: bool,
    remand_target: Option<ObjectId>,
}
impl RemandPolicy {
    fn new(player: PlayerId, remand_can_select_spell_target: bool) -> Self {
        Self {
            player,
            remand_can_select_spell_target,
            target_cast: false,
            remand_target: None,
        }
    }
}
impl CodePolicy for RemandPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.remand-counter-and-win.v1"
        } else {
            "rav.remand-target-watchwolf.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(PILOT_THREAT_ID) && card.can_attack)
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

        if self.player == PlayerId(0)
            && let Some(target) = view
                .stack_spells
                .iter()
                .find(|spell| spell.definition == Some(TARGET_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = ManaCost::with_colors(1, [Color::Blue]);
            if let Some(mana_selection) = payment(view, &cost) {
                let targets = if self.remand_can_select_spell_target {
                    self.remand_target = Some(target.id);
                    vec![Target::Spell(target.id)]
                } else {
                    vec![]
                };
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets,
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }

        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                if let Some(land) = choose_land(view, self.player) {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            if self.player == PlayerId(1) && !self.target_cast && view.turn >= 12 {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                {
                    let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                    if let Some(mana_selection) = payment(view, &cost) {
                        self.target_cast = true;
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
            if self.player == PlayerId(0) && view.turn >= 13 {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(PILOT_THREAT_ID))
                {
                    let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                    if let Some(mana_selection) = payment(view, &cost) {
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
            if view.lands_played == 0 {
                if let Some(land) = choose_land(view, self.player) {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn choose_land<'a>(
    view: &'a GameView,
    player: PlayerId,
) -> Option<&'a cardbench_magic_engine::CardView> {
    let required: &[Color] = if player == PlayerId(0) {
        &[Color::Blue, Color::Green, Color::White]
    } else {
        &[Color::Green, Color::White]
    };
    for color in required {
        if !view.own_battlefield.iter().any(|permanent| {
            permanent.card_types.contains(&CardType::Land) && permanent.mana_colors.contains(color)
        }) && let Some(land) = view.hand.iter().find(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(color)
        }) {
            return Some(land);
        }
    }
    view.hand
        .iter()
        .find(|card| card.card_types.contains(&CardType::Land))
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
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
    let mut need = None;
    for color in &cost.colored {
        let required = cost
            .colored
            .iter()
            .filter(|candidate| *candidate == color)
            .count() as u8;
        if view.mana_pool.amount(*color) < required {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Blue);
    }
    let preferred = need?;
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&preferred)
        })
        .or_else(|| {
            view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && !card.mana_colors.is_empty()
            })
        })?;
    let color = if land.mana_colors.contains(&preferred) {
        preferred
    } else {
        *land.mana_colors.iter().next()?
    };
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let remand = c.remand.ok_or("Remand was not cast")?;
    let target = c.target.ok_or("opposing Watchwolf spell was not cast")?;
    let drawn = c.drawn;
    if c.owners
        != [
            Some(PlayerId(0)),
            Some(PlayerId(1)),
            drawn.map(|_| PlayerId(0)),
        ]
    {
        return Err(format!(
            "Remand, target, or draw owner differs: {:?}",
            c.owners
        ));
    }
    if c.zones[0] != Some(Zone::Graveyard) {
        return Err("Remand did not finish in its controller's graveyard".to_owned());
    }
    let target_counter = event_index(
        &c.events,
        |event| matches!(event, GameEvent::SpellCountered { card, source } if *card == target && *source == remand),
    );
    let target_moved = event_index(
        &c.events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand, .. } if *card == target),
    );
    if target_counter.is_none() {
        return Err("Remand did not counter the exact target spell".to_owned());
    }
    if c.remand_target != Some(target) {
        let opposing_casts = c
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } => Some(*card),
                _ => None,
            })
            .collect::<Vec<_>>();
        let counters = c
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellCountered { card, source } => Some((*card, *source)),
                _ => None,
            })
            .collect::<Vec<_>>();
        return Err(format!(
            "Remand did not target the exact opposing Watchwolf stack object: selected {:?}, first cast target {:?}, P1 spell casts {:?}, counters {:?}",
            c.remand_target, target, opposing_casts, counters
        ));
    }
    if c.zones[1] != Some(Zone::Hand) || target_moved.is_none() {
        return Err(format!(
            "Remand countered the target to the wrong destination: {:?}",
            c.zones[1]
        ));
    }
    if drawn.is_none() || c.zones[2] != Some(Zone::Hand) {
        return Err(format!(
            "Remand did not draw exactly one card for its controller (drawn {:?}, zone {:?})",
            c.drawn, c.zones[2]
        ));
    }
    let drawn = drawn.expect("checked above");
    if drawn == remand || drawn == target || c.owners[2] != Some(PlayerId(0)) {
        return Err("Remand's draw is not a distinct card owned by its controller".to_owned());
    }
    if c.remand_payment.len() != 2 || !c.remand_payment.contains(&Color::Blue) {
        return Err(format!(
            "Remand did not pay its actual {{1}}{{U}} cost: {:?}",
            c.remand_payment
        ));
    }
    if c.target_payment.len() != 2
        || !c.target_payment.contains(&Color::Green)
        || !c.target_payment.contains(&Color::White)
    {
        return Err(format!(
            "target Watchwolf did not pay {{G}}{{W}}: {:?}",
            c.target_payment
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life[0] <= 0
        || c.life[1] > 0
        || c.rejected != 0
    {
        return Err(format!(
            "Remand duel did not reach a clean seat-zero terminal win: {:?}",
            c.life
        ));
    }
    let target_payment = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == target && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("target spell colored payment receipt missing")?;
    let target_cast = event_index_after(&c.events, target_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target)).ok_or("target Watchwolf SpellCast receipt missing")?;
    let remand_payment = event_index_after(&c.events, target_cast, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == remand && colors.len() == 2 && colors.contains(&Color::Blue))).ok_or("Remand's {1}{U} payment receipt missing")?;
    let remand_cast = event_index_after(&c.events, remand_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == remand)).ok_or("Remand SpellCast receipt missing")?;
    let replacement = event_index_after(&c.events, remand_cast, |event| matches!(event, GameEvent::CounteredSpellHandReplacement { card, source } if *card == target && *source == remand)).ok_or("counter-to-hand replacement receipt missing")?;
    let countered = event_index_after(&c.events, replacement, |event| matches!(event, GameEvent::SpellCountered { card, source } if *card == target && *source == remand)).ok_or("Remand did not counter its exact target after being cast")?;
    let moved_target = event_index_after(&c.events, countered, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand, .. } if *card == target)).ok_or("countered target did not move into its owner's hand")?;
    let moved_draw = event_index_after(&c.events, moved_target, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand, .. } if *card == drawn)).ok_or("Remand draw movement receipt missing")?;
    let resolved = event_index_after(
        &c.events,
        moved_draw,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == remand),
    )
    .ok_or("Remand did not resolve after its instructions")?;
    let moved_remand = event_index_after(&c.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == remand)).ok_or("resolved Remand did not move to its owner's graveyard")?;
    if event_index(
        &c.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == target),
    )
    .is_some()
    {
        return Err("countered target spell incorrectly resolved".to_owned());
    }
    let pilot = c.pilot_threats.iter().copied().find(|threat| {
        event_index_after(&c.events, moved_remand, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(threat))).is_some()
    }).ok_or("seat zero Watchwolf never attacked after the Remand response")?;
    let attack = event_index_after(&c.events, moved_remand, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&pilot))).ok_or("pilot Watchwolf attack receipt missing")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == pilot && *amount > 0)).ok_or("pilot Watchwolf did not deal combat damage")?;
    let ended = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal game result event missing")?;
    let ordered = vec![
        target_payment,
        target_cast,
        remand_payment,
        remand_cast,
        replacement,
        countered,
        moved_target,
        moved_draw,
        resolved,
        moved_remand,
        attack,
        damage,
        ended,
    ];
    if !ordered.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(
            "Remand counter, hand return, draw, resolution, and combat event order differs"
                .to_owned(),
        );
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = ordered
        .iter()
        .map(|index| projected_event(&projected[*index]))
        .collect();
    Ok(json!({
        "id": "rav-remand-counter-return-and-draw",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.remand-counter-and-win.v1", "rav.remand-target-watchwolf.v1"],
        "terminal": c.terminal,
        "winner_seat": c.winner,
        "turns": c.turns,
        "accepted_moves": c.accepted,
        "rejected_moves": c.rejected,
        "digest": digest(&c.events),
        "remand_object": remand.0,
        "target_spell_object": target.0,
        "remand_target_object": c.remand_target.map(|object| object.0),
        "drawn_card_object": drawn.0,
        "remand_payment": {"colors": c.remand_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "blue_paid": c.remand_payment.contains(&Color::Blue)},
        "target_spell_payment": {"colors": c.target_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "green_paid": c.target_payment.contains(&Color::Green), "white_paid": c.target_payment.contains(&Color::White)},
        "remand_zone": format!("{:?}", c.zones[0]),
        "target_spell_zone": format!("{:?}", c.zones[1]),
        "drawn_card_zone": format!("{:?}", c.zones[2]),
        "target_spell_owner_seat": c.owners[1].map(|player| player.0),
        "drawn_card_owner_seat": c.owners[2].map(|player| player.0),
        "ordered_event_sequence": ordered,
        "events": events,
        "passed": true
    }))
}

fn projected_event(event: &ProjectedEvent) -> Value {
    json!({
        "sequence": event.sequence,
        "turn": event.turn,
        "kind": event.kind,
        "facts": event.facts,
        "canonical": event.canonical
    })
}

fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn definition_identity_valid(card: &CardDefinition) -> bool {
    card.id == CARD_ID
        && card.name == "Remand"
        && card.set_code == "RAV"
        && card.mana_cost == ManaCost::with_colors(1, [Color::Blue])
        && card.colors == BTreeSet::from([Color::Blue])
        && card.mana_colors.is_empty()
        && card.card_types == BTreeSet::from([CardType::Instant])
        && !card.is_basic_land
        && card.supported_rules
            == [
                "full-rules-fidelity",
                "counter-target-spell-then-draw-controller",
                "countered-spell-to-owner-hand",
            ]
        && card.power.is_none()
        && card.toughness.is_none()
        && card.keywords.is_empty()
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Remand source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Remand source is not valid Rust syntax".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Remand source contains an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Remand source contains an unsupported import".to_owned());
                };
                actual_imports.extend(paths);
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => return Err("Remand source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err("Remand imports or module items differ from its task boundary".to_owned());
    }
    let function = definition.ok_or("Remand definition() is missing")?;
    if !function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    }) || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
        || function.sig.ident != "definition"
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.abi.is_some()
        || !matches!(&function.sig.output, ReturnType::Type(_, ty) if type_path_is(ty.as_ref(), &["CardDefinition"]))
        || function.block.stmts.len() != 1
    {
        return Err("Remand must export a plain definition() -> CardDefinition".to_owned());
    }
    let Stmt::Expr(Expr::Struct(card), None) = &function.block.stmts[0] else {
        return Err("Remand definition must return a direct CardDefinition literal".to_owned());
    };
    if !path_is(&card.path, &["CardDefinition"]) || card.rest.is_some() || card.fields.len() != 13 {
        return Err("Remand must use all named CardDefinition fields".to_owned());
    }
    let mut fields = BTreeMap::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Remand CardDefinition fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Remand CardDefinition contains shorthand or duplicate fields".to_owned());
        }
    }
    let names: BTreeSet<_> = [
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
    ]
    .into_iter()
    .collect();
    if fields.keys().map(String::as_str).collect::<BTreeSet<_>>() != names {
        return Err("Remand CardDefinition fields differ from the public card schema".to_owned());
    }
    check_string(fields["id"], CARD_ID)?;
    check_string(fields["name"], "Remand")?;
    check_string(fields["set_code"], "RAV")?;
    check_mana_cost(fields["mana_cost"])?;
    check_set(fields["colors"], &["Color", "Blue"])?;
    check_empty_set(fields["mana_colors"])?;
    check_set(fields["card_types"], &["CardType", "Instant"])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_none(fields["power"])?;
    check_none(fields["toughness"])?;
    check_empty_vec(fields["keywords"])?;
    check_allowed_effects(fields["effects"])?;
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
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
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_path_is(ty: &Type, expected: &[&str]) -> bool {
    match ty {
        Type::Path(path) => path_is(&path.path, expected),
        _ => false,
    }
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(segment.arguments, PathArguments::None)
            })
}
fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    match expr {
        Expr::Lit(value) if value.attrs.is_empty() => match &value.lit {
            Lit::Str(value) if value.value() == expected => Ok(()),
            _ => Err(format!("Remand string field must be {expected:?}")),
        },
        _ => Err(format!("Remand string field must be {expected:?}")),
    }
}
fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    match expr {
        Expr::Lit(value)
            if value.attrs.is_empty()
                && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected) =>
        {
            Ok(())
        }
        _ => Err("Remand basic-land flag differs".to_owned()),
    }
}
fn call_arguments(expr: &Expr, path: &[&str]) -> Option<Vec<Expr>> {
    let Expr::Call(call) = expr else { return None };
    let Expr::Path(function) = call.func.as_ref() else {
        return None;
    };
    if !path_is(&function.path, path) {
        return None;
    }
    Some(call.args.iter().cloned().collect())
}
fn check_set(expr: &Expr, color_path: &[&str]) -> Result<(), String> {
    let args = call_arguments(expr, &["BTreeSet", "from"])
        .ok_or("Remand color/type set must use BTreeSet::from")?;
    if args.len() != 1 {
        return Err("Remand set constructor arity differs".to_owned());
    }
    let Expr::Array(array) = &args[0] else {
        return Err("Remand set must use a literal array".to_owned());
    };
    if array.elems.len() != 1
        || !matches!(&array.elems[0], Expr::Path(path) if path_is(&path.path, color_path))
    {
        return Err("Remand color/type set values differ".to_owned());
    }
    Ok(())
}
fn check_empty_set(expr: &Expr) -> Result<(), String> {
    match call_arguments(expr, &["BTreeSet", "new"]) {
        Some(args) if args.is_empty() => Ok(()),
        _ => Err("Remand mana colors must be an empty set".to_owned()),
    }
}
fn check_mana_cost(expr: &Expr) -> Result<(), String> {
    let args = call_arguments(expr, &["ManaCost", "with_colors"])
        .ok_or("Remand mana cost must use ManaCost::with_colors")?;
    if args.len() != 2
        || !matches!(&args[0], Expr::Lit(lit) if lit.attrs.is_empty() && matches!(&lit.lit, Lit::Int(number) if number.base10_parse::<u64>().ok()==Some(1)))
    {
        return Err("Remand must cost {1}{U}".to_owned());
    }
    let Expr::Array(array) = &args[1] else {
        return Err("Remand colored cost must use a literal color array".to_owned());
    };
    if array.elems.len() == 1
        && matches!(&array.elems[0], Expr::Path(path) if path_is(&path.path, &["Color", "Blue"]))
    {
        Ok(())
    } else {
        Err("Remand must cost {1}{U}".to_owned())
    }
}
fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Remand supported rules must be a borrowed static slice".to_owned());
    };
    if reference.mutability.is_some() {
        return Err("Remand supported rules cannot be mutable".to_owned());
    }
    let Expr::Array(array) = reference.expr.as_ref() else {
        return Err("Remand supported rules must be a static array".to_owned());
    };
    let expected = [
        "full-rules-fidelity",
        "counter-target-spell-then-draw-controller",
        "countered-spell-to-owner-hand",
    ];
    if array.elems.len() != expected.len() {
        return Err("Remand supported rules differ".to_owned());
    }
    for (expr, expected) in array.elems.iter().zip(expected) {
        check_string(expr, expected)?;
    }
    Ok(())
}
fn check_none(expr: &Expr) -> Result<(), String> {
    match expr {
        Expr::Path(path) if path_is(&path.path, &["None"]) => Ok(()),
        _ => Err("Remand instant must not declare creature power or toughness".to_owned()),
    }
}
fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    match expr {
        Expr::Macro(mac)
            if mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && mac.mac.tokens.is_empty() =>
        {
            Ok(())
        }
        _ => Err("Remand keyword list must be empty".to_owned()),
    }
}
fn check_allowed_effects(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Remand effects must use a literal vec!".to_owned());
    };
    if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
        return Err("Remand effects must use a literal vec!".to_owned());
    }
    if mac.mac.tokens.is_empty() {
        return Ok(());
    }
    let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
    let effects = parser
        .parse2(mac.mac.tokens.clone())
        .map_err(|_| "Remand effects are not a literal sequence".to_owned())?;
    for effect in effects {
        let Expr::Path(path) = effect else {
            return Err("Remand effects must be direct enum variants".to_owned());
        };
        if ![
            "CounterTargetSpellToOwnersHand",
            "CounterTargetSpell",
            "DrawController",
        ]
        .iter()
        .any(|variant| path_is(&path.path, &["Effect", variant]))
        {
            return Err("Remand contains an unsupported semantic operation".to_owned());
        }
    }
    Ok(())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
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

fn digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for event in events {
        for byte in format!("{event:?}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}
