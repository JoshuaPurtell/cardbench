//! Full-game RAV Devouring Light trace for Convoke and attacking-creature exile.

use cardbench_magic_engine::{
    CardType, Color, ConvokeContribution, ConvokePayment, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DEVOURING-LIGHT";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/devouring_light.rs";
const P0_DECK: &str = "rav_devouring_light_convoke_answer";
const P1_DECK: &str = "rav_devouring_light_single_attacker";
const TRACE_SEED: u64 = 15;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/devouring_light.rs"]
mod candidate_devouring_light;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Devouring Light PvP result: {error}");
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
            eprintln!("Devouring Light PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
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
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err(
            "Devouring Light seeded combat game did not replay deterministically".to_owned(),
        );
    }
    let no_convoke = mutation_detected(seed, Mutation::RemoveConvoke, "convoke")?;
    let broad_target = mutation_detected(
        seed,
        Mutation::BroadenTarget,
        "target was not an attacking creature",
    )?;
    if !no_convoke || !broad_target {
        return Err("Devouring Light semantic mutation escaped the full-game trace".to_owned());
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
        "card": {"collector_number": 13, "definition_id": CARD_ID, "name": "Devouring Light"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: white identity and colored payment",
                "117: priority and instant-speed casting",
                "301.1: instant cards",
                "601.2: cast announcement, target legality, and cost payment",
                "608.2b and 608.2c: target legality on resolution",
                "701.11: exile and zone changes",
                "508 and 510: attacker declaration and combat",
                "702.50: Convoke payment"
            ],
            "source_contract": "RAV Devouring Light identity, {1}{W}{W}, Convoke, and exile of a creature that is attacking or blocking",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity": {
            "convoke_removed_mutation_detected_in_game": no_convoke,
            "attacking_or_blocking_target_broadened_mutation_detected_in_game": broad_target,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveConvoke,
    BroadenTarget,
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    light: Option<ObjectId>,
    target: Option<ObjectId>,
    target_was_attacking_at_cast: Option<bool>,
    cast_step: Option<Step>,
    cast_active_player: Option<PlayerId>,
    cast_priority: Option<PlayerId>,
    light_cast_payment: Vec<Color>,
    convokers: Vec<ObjectId>,
    convoke_colors: Vec<Color>,
    convokers_tapped_after_cast: Vec<bool>,
    target_zone: Option<Zone>,
    caster_life: Option<i64>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let captured = run_match(TRACE_SEED, Mutation::None)?;
    validate_trace(&captured, TRACE_SEED)?;
    Ok((TRACE_SEED, captured))
}

fn mutation_detected(seed: u64, mutation: Mutation, diagnostic: &str) -> Result<bool, String> {
    match (mutation, run_match(seed, mutation)) {
        (Mutation::RemoveConvoke, Err(_)) => Ok(true),
        (Mutation::RemoveConvoke, Ok(captured)) => Ok(validate_trace(&captured, seed).is_err()),
        (Mutation::BroadenTarget, Ok(captured)) => Ok(validate_trace(&captured, seed)
            .err()
            .is_some_and(|error| error.contains(diagnostic))),
        (Mutation::BroadenTarget, Err(_)) | (Mutation::None, _) => Ok(false),
    }
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.terminal == right.terminal
        && left.light == right.light
        && left.target == right.target
        && left.target_was_attacking_at_cast == right.target_was_attacking_at_cast
        && left.cast_step == right.cast_step
        && left.cast_active_player == right.cast_active_player
        && left.cast_priority == right.cast_priority
        && left.light_cast_payment == right.light_cast_payment
        && left.convokers == right.convokers
        && left.convoke_colors == right.convoke_colors
        && left.convokers_tapped_after_cast == right.convokers_tapped_after_cast
        && left.target_zone == right.target_zone
        && left.caster_life == right.caster_life
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-COURIER-HAWK".to_owned(),
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
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
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
    for (name, deck) in [(P0_DECK, &pilot_deck), (P1_DECK, &opponent_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Devouring Light is missing from the RAV registry")?;
    match mutation {
        Mutation::None => {}
        Mutation::RemoveConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::BroadenTarget => definition.effects = vec![Effect::ExileTargetCreature],
    }
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 40,
        max_policy_moves: 4_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DevouringPolicy::new(PlayerId(0), true, mutation)),
        Box::new(DevouringPolicy::new(PlayerId(1), false, Mutation::None)),
    ];
    let mut capture = CapturedMatch {
        events: vec![],
        winner: None,
        terminal: String::new(),
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        light: None,
        target: None,
        target_was_attacking_at_cast: None,
        cast_step: None,
        cast_active_player: None,
        cast_priority: None,
        light_cast_payment: vec![],
        convokers: vec![],
        convoke_colors: vec![],
        convokers_tapped_after_cast: vec![],
        target_zone: None,
        caster_life: None,
    };
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Devouring Light match exceeded its development bound at turn {} after {} moves (P0 life {}, P1 life {}, P0 attacks {})",
                game.turn,
                capture.accepted_moves,
                game.player(PlayerId(0)).map_or(0, |player| player.life),
                game.player(PlayerId(1)).map_or(0, |player| player.life),
                game.event_log.iter().filter(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if !attackers.is_empty())).count()
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("missing policy seat")?;
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
        let before = game.event_log.len();
        attempted += 1;
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            capture.light = Some(request.card);
            capture.target = request.targets.first().and_then(|target| match target {
                Target::Permanent(object) => Some(*object),
                _ => None,
            });
            capture.target_was_attacking_at_cast = capture.target.map(|target| {
                view.combat_attackers
                    .iter()
                    .any(|attacker| attacker.id == target && attacker.controller == PlayerId(1))
            });
            capture.cast_step = Some(view.step);
            capture.cast_active_player = Some(view.active_player);
            capture.cast_priority = Some(view.priority);
        }
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => capture.accepted_moves += 1,
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        for event in &game.event_log[before..] {
            match event {
                GameEvent::AttackersDeclared {
                    player: PlayerId(1),
                    attackers,
                } if capture.target.is_none() => {
                    capture.target = attackers.iter().copied().find(|attacker| {
                        game.card_definition(*attacker)
                            .is_ok_and(|definition| definition.id == WATCHWOLF_ID)
                    });
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    capture.light = Some(*card);
                    capture.light_cast_payment = colors.clone();
                }
                GameEvent::ConvokeUsed {
                    player: PlayerId(0),
                    creature,
                    contribution,
                } if capture.light.is_some() => {
                    capture.convokers.push(*creature);
                    if let Some(color) = contribution {
                        capture.convoke_colors.push(*color);
                    }
                    capture.convokers_tapped_after_cast.push(
                        game.object(*creature)
                            .map_err(|error| error.to_string())?
                            .tapped,
                    );
                }
                GameEvent::SpellResolved { card } if Some(*card) == capture.light => {}
                GameEvent::CardMoved {
                    card,
                    to: Zone::Exile,
                } if Some(*card) == capture.target => capture.target_zone = Some(Zone::Exile),
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.terminal = capture.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    capture.turns = game.turn;
    capture.rejected_moves = attempted.saturating_sub(capture.accepted_moves);
    capture.caster_life = game.player(PlayerId(0)).ok().map(|player| player.life);
    Ok(capture)
}

struct DevouringPolicy {
    player: PlayerId,
    pilot: bool,
    mutation: Mutation,
    light_cast: bool,
    light_attempted: bool,
    has_attacked: bool,
}
impl DevouringPolicy {
    fn new(player: PlayerId, pilot: bool, mutation: Mutation) -> Self {
        Self {
            player,
            pilot,
            mutation,
            light_cast: false,
            light_attempted: false,
            has_attacked: false,
        }
    }
    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
    }
    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && preferred.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = preferred
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
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
    fn cast_light(&mut self, view: &GameView, target: ObjectId) -> Option<PolicyAction> {
        let creatures: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Creature)
                    && !card.tapped
                    && card.colors.contains(&Color::White)
            })
            .take(2)
            .collect();
        if creatures.len() != 2 {
            return None;
        }
        let Some(generic_color) = Color::MANA_ALL
            .into_iter()
            .find(|color| view.mana_pool.amount(*color) > 0)
        else {
            return Self::mana_source(view, None);
        };
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        self.light_attempted = true;
        self.light_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(target)],
                convoke: creatures
                    .iter()
                    .map(|creature| ConvokePayment {
                        creature: creature.id,
                        contribution: ConvokeContribution::Color(Color::White),
                    })
                    .collect(),
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![generic_color],
                hybrid: vec![],
            },
        })
    }
    fn cast_creature(&self, view: &GameView, definition: &'static str) -> Option<PolicyAction> {
        let (cost, _) = match definition {
            "RAV-WATCHWOLF" => (ManaCost::with_colors(0, [Color::Green, Color::White]), 0),
            "RAV-COURIER-HAWK" => (ManaCost::with_colors(1, [Color::White]), 0),
            _ => return None,
        };
        if let Some(selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(definition))?;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(Color::White)
            });
        Self::mana_source(view, missing)
    }
}
impl CodePolicy for DevouringPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.devouring-light-convoke-answer.v1"
        } else {
            "rav.single-attacker-pressure.v1"
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
            let attackers = if self.pilot && self.light_cast {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else if !self.pilot {
                if self.has_attacked {
                    vec![]
                } else {
                    let attackers: Vec<_> = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                        .map(|card| vec![card.id])
                        .unwrap_or_default();
                    self.has_attacked = !attackers.is_empty();
                    attackers
                }
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
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if self.pilot
            && !self.light_cast
            && view.active_player != self.player
            && view.step == Step::DeclareAttackers
            && view.attackers_declared
        {
            if let Some(target) = view.combat_attackers.iter().find(|attacker| {
                attacker.controller != self.player && attacker.definition == Some(WATCHWOLF_ID)
            }) {
                if Self::total_mana(view) == 0 {
                    if let Some(action) = Self::mana_source(view, None) {
                        return action;
                    }
                }
                if let Some(action) = self.cast_light(view, target.id) {
                    return action;
                }
            }
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.pilot {
            if self.mutation == Mutation::BroadenTarget
                && !self.light_attempted
                && self.player == PlayerId(0)
            {
                if let Some(target) = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID))
                {
                    if let Some(action) = self.cast_light(view, target.id) {
                        return action;
                    }
                }
            }
            let creatures = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .count();
            if creatures < 2 {
                for id in [WATCHWOLF_ID, "RAV-COURIER-HAWK"] {
                    if view.hand.iter().any(|card| card.definition == Some(id)) {
                        if let Some(action) = self.cast_creature(view, id) {
                            return action;
                        }
                    }
                }
            }
        } else if !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = self.cast_creature(view, WATCHWOLF_ID) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let light = captured.light.ok_or("Devouring Light was not cast")?;
    let target = captured
        .target
        .ok_or("opposing Watchwolf target was not established")?;
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Devouring Light full game did not reach a seat-zero terminal win".to_owned());
    }
    let pilot_damage_to_opponent: i32 = captured
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                amount,
                ..
            } => Some(*amount),
            _ => None,
        })
        .sum();
    if pilot_damage_to_opponent < 20 {
        return Err(format!(
            "Devouring Light full game did not finish by P0 combat damage (dealt {pilot_damage_to_opponent})"
        ));
    }
    if captured.target_was_attacking_at_cast != Some(true)
        || captured.cast_step != Some(Step::DeclareAttackers)
        || captured.cast_active_player != Some(PlayerId(1))
        || captured.cast_priority != Some(PlayerId(0))
    {
        return Err(
            "Devouring Light target was not an attacking creature at instant-speed cast".to_owned(),
        );
    }
    if captured.target_zone != Some(Zone::Exile) {
        return Err("Devouring Light did not exile its attacking target".to_owned());
    }
    if captured.convokers.len() != 2
        || captured.convoke_colors != [Color::White, Color::White]
        || captured.convokers_tapped_after_cast != [true, true]
    {
        return Err(format!(
            "Devouring Light must tap two white creatures to pay its white symbols: {:?} {:?} {:?}",
            captured.convokers, captured.convoke_colors, captured.convokers_tapped_after_cast
        ));
    }
    if captured.light_cast_payment.len() != 1 {
        return Err(format!(
            "Devouring Light generic mana payment differs: {:?}",
            captured.light_cast_payment
        ));
    }
    let attacker = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&target))).ok_or("target was not declared as an attacker")?;
    let paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == light)).ok_or("Devouring Light mana payment receipt missing")?;
    let cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == light)).ok_or("Devouring Light cast receipt missing")?;
    let resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == light),
    )
    .ok_or("Devouring Light resolution receipt missing")?;
    let exile = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if *card == target),
    )
    .ok_or("attacking Watchwolf exile zone-change receipt missing")?;
    let end = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal receipt missing")?;
    if !(attacker < paid && paid < cast && cast < exile && exile < resolved && resolved < end) {
        return Err(format!(
            "attack declaration, instant cast, resolution, exile, and terminal event order differs (attack={attacker}, paid={paid}, cast={cast}, resolved={resolved}, exile={exile}, end={end})"
        ));
    }
    let convoke_indices: Vec<_> = captured.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: Some(Color::White) } if captured.convokers.contains(creature)).then_some(index)).collect();
    if convoke_indices.len() != 2
        || convoke_indices
            .iter()
            .any(|index| *index < attacker || *index > cast)
    {
        return Err("both Convoke payments must occur during the attacking-target cast".to_owned());
    }
    let events = relevant_events(&captured.events, light, target, &captured.convokers);
    let event_kinds: Vec<_> = events.iter().map(|event| event.kind.as_str()).collect();
    if !contains_ordered_subsequence(
        &event_kinds,
        &[
            "AttackersDeclared",
            "SpellManaPaid",
            "SpellCast",
            "CardMoved",
            "SpellResolved",
            "GameEnded",
        ],
    ) {
        return Err(format!(
            "Devouring Light projected event sequence differs: {event_kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-devouring-light-convoke-exile-attacking-creature",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.devouring-light-convoke-answer.v1", "rav.single-attacker-pressure.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "devouring_light_object": light.0,
        "attacking_target_object": target.0,
        "target_controller": 1,
        "target_was_attacking_at_cast": true,
        "cast_step": format!("{:?}", captured.cast_step.unwrap()),
        "printed_cost": "{1}{W}{W}",
        "mana_spent_for_generic": captured.light_cast_payment.iter().map(|color| format!("{:?}", color)).collect::<Vec<_>>(),
        "convoke_white_symbols_paid": captured.convoke_colors.len(),
        "convoke_creatures": captured.convokers.iter().map(|object| object.0).collect::<Vec<_>>(),
        "convokers_tapped_as_cost": captured.convokers_tapped_after_cast,
        "target_zone_after_resolution": format!("{:?}", captured.target_zone.unwrap()),
        "pilot_damage_to_opponent": pilot_damage_to_opponent,
        "pilot_life_at_terminal": captured.caster_life,
        "events": events,
        "passed": true
    }))
}

fn relevant_events(
    events: &[GameEvent],
    light: ObjectId,
    target: ObjectId,
    convokers: &[ObjectId],
) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .filter(|event| {
            if event.kind == "GameEnded" {
                return true;
            }
            if event.kind == "AttackersDeclared"
                && event.canonical.contains(&format!("ObjectId({})", target.0))
            {
                return true;
            }
            if event.kind == "ConvokeUsed"
                && convokers
                    .iter()
                    .any(|id| event.canonical.contains(&format!("ObjectId({})", id.0)))
            {
                return true;
            }
            if event.kind == "SpellManaPaid"
                || event.kind == "SpellCast"
                || event.kind == "SpellResolved"
            {
                return event.canonical.contains(&format!("ObjectId({})", light.0));
            }
            event.kind == "CardMoved"
                && event.canonical.contains(&format!("ObjectId({})", target.0))
                && event.canonical.contains("to: Exile")
        })
        .collect()
}
fn contains_ordered_subsequence(actual: &[&str], expected: &[&str]) -> bool {
    let mut next = 0;
    for item in actual {
        if expected.get(next) == Some(item) {
            next += 1;
        }
        if next == expected.len() {
            return true;
        }
    }
    false
}
fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_digest(events: &[GameEvent]) -> String {
    let text = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in text.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
fn card_status(card: &CatalogCard) -> Value {
    let (status, id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":id,"name":card.name,"engine_status":status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
        ),
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("reward.txt"),
        format!("{:.8}\n", report["harbor_reward"].as_f64().unwrap_or(0.0)),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_devouring_light::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Devouring Light is missing from the RAV catalog")?;
    if submitted != registered
        || submitted.id != CARD_ID
        || submitted.name != "Devouring Light"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(1, [Color::White, Color::White])
        || submitted.colors != BTreeSet::from([Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Instant])
        || submitted.is_basic_land
        || submitted.power.is_some()
        || submitted.toughness.is_some()
        || submitted.keywords != vec![Keyword::Convoke]
        || submitted.effects != vec![Effect::ExileTargetPermanent]
        || submitted
            .effects
            .first()
            .and_then(Effect::target_requirement)
            != Some(TargetRequirement::AttackingOrBlockingCreature)
    {
        return Err("Devouring Light identity, {1}{W}{W}, Convoke, or attacking/blocking exile effect differs".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Devouring Light source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Devouring Light source is not declarative Rust".to_owned())?;
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
    .map(|parts| parts.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut imports)?
            }
            Item::Fn(function) if function.sig.ident == "definition" && definition.is_none() => {
                definition = Some(function)
            }
            _ => {
                return Err(
                    "Devouring Light module contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || definition.is_none() || file.items.len() != 3 {
        return Err("Devouring Light candidate module imports or item count differ".to_owned());
    }
    let function = definition.unwrap();
    let safe_attrs = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let valid_return = matches!(&function.sig.output,syn::ReturnType::Type(_,ty) if type_is(ty,&["CardDefinition"]));
    let vis = matches!(&function.vis,Visibility::Restricted(vis) if vis.path.is_ident("crate"));
    if !safe_attrs || !valid_function_shape(function, "definition", valid_return, vis) {
        return Err("Devouring Light definition must be a literal-only crate function".to_owned());
    }
    let Stmt::Expr(Expr::Struct(definition), None) = &function.block.stmts[0] else {
        return Err("Devouring Light must return a direct CardDefinition literal".to_owned());
    };
    if !path_is(&definition.path, &["CardDefinition"])
        || definition.rest.is_some()
        || definition.fields.len() != 13
    {
        return Err(
            "Devouring Light source identity must use a safe direct definition literal".to_owned(),
        );
    }
    if let Some(field) = definition
        .fields
        .iter()
        .find(|field| !safe_expr(&field.expr))
    {
        let field_name = match &field.member {
            syn::Member::Named(name) => name.to_string(),
            syn::Member::Unnamed(index) => index.index.to_string(),
        };
        return Err(format!(
            "Devouring Light source field {field_name} must use a safe direct literal"
        ));
    }
    Ok(())
}
fn valid_function_shape(function: &syn::ItemFn, name: &str, valid_return: bool, vis: bool) -> bool {
    vis && valid_return
        && function.sig.ident == name
        && function.sig.constness.is_none()
        && function.sig.asyncness.is_none()
        && function.sig.unsafety.is_none()
        && function.sig.abi.is_none()
        && function.sig.generics.params.is_empty()
        && function.sig.generics.where_clause.is_none()
        && function.sig.inputs.is_empty()
        && function.sig.variadic.is_none()
        && function.block.stmts.len() == 1
}
fn safe_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            value.attrs.is_empty()
                && value.qself.is_none()
                && [
                    &["Color", "White"][..],
                    &["CardType", "Instant"],
                    &["Keyword", "Convoke"],
                    &["Effect", "ExileTargetPermanent"],
                ]
                .iter()
                .any(|path| path_is(&value.path, path))
                || value.attrs.is_empty()
                    && value.qself.is_none()
                    && path_is(&value.path, &["None"])
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                &["BTreeSet", "from"][..],
                &["BTreeSet", "new"],
                &["ManaCost", "with_colors"],
            ]
            .iter()
            .any(|expected| path_is(&path.path, expected))
        }
        Expr::Struct(value) => {
            !value.attrs.is_empty() == false
                && value.rest.is_none()
                && [&["CardDefinition"][..]]
                    .iter()
                    .any(|expected| path_is(&value.path, expected))
                && value.fields.iter().all(|field| {
                    field.attrs.is_empty()
                        && matches!(field.member, syn::Member::Named(_))
                        && field.colon_token.is_some()
                        && safe_expr(&field.expr)
                })
        }
        Expr::Macro(value) => {
            if !value.attrs.is_empty() || !value.mac.path.is_ident("vec") {
                return false;
            }
            let tokens = value.mac.tokens.to_string().replace(' ', "");
            tokens.is_empty()
                || tokens == "Keyword::Convoke"
                || tokens == "Effect::ExileTargetPermanent"
        }
        _ => false,
    }
}
fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, output)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            output.insert(next);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, output)?;
            }
            Ok(())
        }
        _ => Err("Devouring Light uses an unsupported import".to_owned()),
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
fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty,Type::Path(path)if path.qself.is_none()&&path_is(&path.path,expected))
}
