//! Full-game RAV Greater Mossdog authority for Dredge 3 and its exact return/cast.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GREATER-MOSSDOG";
const CARD_NAME: &str = "Greater Mossdog";
const OPPONENT_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/greater_mossdog.rs";
const DEFAULT_TRACE_SEED: u64 = 12;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/greater_mossdog.rs"]
mod candidate_greater_mossdog;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Greater Mossdog PvP result: {error}");
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
            eprintln!("Greater Mossdog PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    validate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Greater Mossdog shuffled-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "missing_dredge_mutation_detected": mutation_detected(seed, Mutation::MissingDredge),
        "wrong_dredge_count_mutation_detected": mutation_detected(seed, Mutation::DredgeTwo),
        "wrong_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "unprinted_creature_target_trigger_mutation_detected": target_trigger_mutation_detected()?,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "unprinted_creature_target_trigger_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Greater Mossdog semantic mutation escaped: {key}"));
        }
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
        "card": {"collector_number": 169, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and turn-based draw decisions",
                "202.1-202.2: colored mana cost and color identity",
                "302.1-302.2: creature characteristics and combat",
                "601.2: casting and paying for the creature spell",
                "702.52: Dredge replacement of a draw",
                "510: combat damage and creature deaths",
                "704: state-based actions after combat damage"
            ],
            "source_contract": "RAV Greater Mossdog #169 is a green {3}{G} 3/3 creature with Dredge 3; it has no creature-becomes-target trigger",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingDredge,
    DredgeTwo,
    WrongCost,
    WrongBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    life_totals: [i64; 2],
    mossdog: Option<ObjectId>,
    mossdog_pt: Option<(i32, i32)>,
    mossdog_zone: Option<Zone>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let seed = std::env::var("CARDBENCH_GREATER_MOSSDOG_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_TRACE_SEED);
    match run_match(seed, Mutation::None) {
        Ok(captured) => {
            validate_trace(&captured, seed)?;
            Ok((seed, captured))
        }
        Err(error) => Err(error),
    }
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_greater_mossdog::definition();
    match mutation {
        Mutation::None => {}
        Mutation::MissingDredge => definition.keywords.clear(),
        Mutation::DredgeTwo => definition.keywords = vec![Keyword::Dredge(2)],
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Green]),
        Mutation::WrongBody => {
            definition.power = Some(2);
            definition.toughness = Some(3);
        }
    }
    definition
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 29,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 30,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Greater Mossdog missing from the RAV registry")?;
    *registered = definition_for(mutation);
    let mossdog_cost = registered.mana_cost.clone();
    let watchwolf_cost = definitions
        .iter()
        .find(|card| card.id == OPPONENT_ID)
        .map(|card| card.mana_cost.clone())
        .ok_or("Watchwolf missing from the RAV registry")?;
    let triggers = rav_triggered_ability_bindings();
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
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
        Box::new(MossdogPolicy::new(PlayerId(0), true, mossdog_cost.clone(), watchwolf_cost.clone())),
        Box::new(MossdogPolicy::new(PlayerId(1), false, mossdog_cost, watchwolf_cost)),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Greater Mossdog PvP match exceeded its development bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} / {:?} for {:?}: {error}",
                    view.turn, view.step, player
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let mossdog = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } => Some(*card),
        _ => None,
    });
    let mossdog_pt = mossdog
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let mossdog_zone = mossdog.and_then(|card| game.zone_of(card));
    let life_totals = [
        game.player(PlayerId(0)).map_err(|error| error.to_string())?.life,
        game.player(PlayerId(1)).map_err(|error| error.to_string())?.life,
    ];
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner,
        turns: game.turn,
        accepted_moves,
        life_totals,
        mossdog,
        mossdog_pt,
        mossdog_zone,
    })
}

struct MossdogPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    mossdog_cost: ManaCost,
    watchwolf_cost: ManaCost,
    initial_source: Option<ObjectId>,
    blocked_source: Option<ObjectId>,
    recovered_source: Option<ObjectId>,
    has_blocked: bool,
}

impl MossdogPolicy {
    fn new(player: PlayerId, pilot: bool, mossdog_cost: ManaCost, watchwolf_cost: ManaCost) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            mossdog_cost,
            watchwolf_cost,
            initial_source: None,
            blocked_source: None,
            recovered_source: None,
            has_blocked: false,
        }
    }
}

impl CodePolicy for MossdogPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.greater-mossdog-dredge-pilot.v1"
        } else {
            "rav.greater-mossdog-watchwolf-opponent.v1"
        }
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.pilot {
            let candidate = self
                .blocked_source
                .and_then(|source| view.dredge_candidates.iter().find(|card| card.id == source))
                .or_else(|| {
                    view.dredge_candidates
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                });
            if let Some(candidate) = candidate {
                self.recovered_source = Some(candidate.id);
                return PolicyAction::Draw {
                    decision: view
                        .draw_replacement_decision
                        .expect("live draw replacement has an identity"),
                    dredge: Some(candidate.id),
                };
            }
        }
        self.inner.propose_draw_replacement(view)
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.pilot {
                self.recovered_source
                    .into_iter()
                    .filter(|source| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == *source && card.can_attack)
                    })
                    .collect()
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(OPPONENT_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.pilot && !self.has_blocked && self.recovered_source.is_none()
            {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(OPPONENT_ID))
                    .map(|card| card.id);
                let blocker = self
                    .initial_source
                    .and_then(|source| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == source && card.definition == Some(CARD_ID))
                            .then_some(source)
                    })
                    .or_else(|| {
                        view.own_battlefield
                            .iter()
                            .find(|card| card.definition == Some(CARD_ID))
                            .map(|card| card.id)
                    });
                match (attacker, blocker) {
                    (Some(attacker), Some(blocker)) => {
                        self.blocked_source = Some(blocker);
                        self.has_blocked = true;
                        vec![CombatBlock { attacker, blocker }]
                    }
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }

        let action = self.inner.propose_move(view);
        let action = match action {
            PolicyAction::Cast(request) => {
                let cost = view
                    .hand
                    .iter()
                    .find(|card| card.id == request.card)
                    .and_then(|card| match card.definition {
                        Some(CARD_ID) => Some(&self.mossdog_cost),
                        Some(OPPONENT_ID) => Some(&self.watchwolf_cost),
                        _ => None,
                    });
                if let Some(selection) = cost.and_then(|cost| payment_selection(view, cost)) {
                    PolicyAction::CastWithPayment {
                        request,
                        chosen_x: None,
                        mana_selection: selection,
                    }
                } else {
                    PolicyAction::Cast(request)
                }
            }
            action => action,
        };
        if self.pilot {
            let cast_source = match &action {
                PolicyAction::Cast(request)
                | PolicyAction::CastWithPayment { request, .. }
                | PolicyAction::CastWithMode { request, .. }
                | PolicyAction::CastWithModeAndColorChoice { request, .. }
                | PolicyAction::CastWithColorChoice { request, .. }
                | PolicyAction::CastWithCreatureSpellAdditionalMana { request, .. } => {
                    Some(request.card)
                }
                _ => None,
            };
            if let Some(source) = cast_source
                && view.hand.iter().any(|card| card.id == source && card.definition == Some(CARD_ID))
            {
                self.initial_source.get_or_insert(source);
            }
        }
        action
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
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

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let events = &captured.events;
    let mossdog = captured.mossdog.ok_or("pilot never cast Greater Mossdog")?;
    let watchwolf = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } => Some(*card),
            _ => None,
        })
        .ok_or("opponent never cast the 3/3 Watchwolf combatant")?;
    let casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == mossdog)
                .then_some(index)
        })
        .collect();
    if casts.len() < 2 {
        return Err("the exact Mossdog returned by Dredge was not recast".to_owned());
    }
    let payments: Vec<Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == mossdog => Some(colors.clone()),
            _ => None,
        })
        .collect();
    if payments.len() < 2
        || payments.iter().take(2).any(|payment| {
            payment.len() != 4 || payment.iter().any(|color| *color != Color::Green)
        })
    {
        return Err(format!("Mossdog spell payments do not show {{3}}{{G}}: {payments:?}"));
    }
    let dredges: Vec<(usize, u8)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::Dredged {
                player: PlayerId(0),
                card,
                count,
            } if *card == mossdog => Some((index, *count)),
            _ => None,
        })
        .collect();
    if dredges.len() != 1 || dredges[0].1 != 3 {
        return Err(format!("expected exactly one Dredge 3 of Mossdog: {dredges:?}"));
    }
    let dredge_index = dredges[0].0;
    let mills = validate_dredge_suffix(events, dredge_index, mossdog)?;
    let block_index = events
        .iter()
        .position(|event| {
            matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments }
                if assignments.iter().any(|(attacker, blocker)| *attacker == watchwolf && *blocker == mossdog))
        })
        .ok_or("Greater Mossdog did not block Watchwolf")?;
    let died_index = events
        .iter()
        .enumerate()
        .skip(block_index + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == mossdog)
                .then_some(index)
        })
        .ok_or("blocked Mossdog did not move to its owner's graveyard")?;
    let resolved_indices: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == mossdog)
                .then_some(index)
        })
        .collect();
    if resolved_indices.len() < 2
        || !(casts[0] < block_index && block_index < died_index && died_index < dredge_index)
        || !(dredge_index < casts[1] && casts[1] < resolved_indices[1])
    {
        return Err("cast, combat death, Dredge replacement, and recast ordering differs".to_owned());
    }
    if !events.iter().any(|event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&mossdog))
    }) {
        return Err("the recovered Mossdog never attacked".to_owned());
    }
    let combat_damage = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == mossdog => Some(*amount),
            _ => None,
        })
        .sum::<i32>();
    if combat_damage < 6 {
        return Err(format!("recast Mossdog combat damage is missing or too small: {combat_damage}"));
    }
    if captured.mossdog_pt != Some((3, 3)) || captured.mossdog_zone != Some(Zone::Battlefield) {
        return Err(format!("terminal Mossdog characteristics/zone differ: pt={:?}, zone={:?}", captured.mossdog_pt, captured.mossdog_zone));
    }
    if events.iter().any(|event| {
        matches!(event, GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mossdog && *ability == "targeted-creature-return-from-graveyard")
    }) {
        return Err("Greater Mossdog unexpectedly put the unprinted target trigger on the stack".to_owned());
    }
    if captured.winner != Some(0) || captured.turns > 80 || captured.accepted_moves > 5_000 {
        return Err(format!(
            "duel did not reach bounded pilot win: winner={:?}, turns={}, moves={}",
            captured.winner, captured.turns, captured.accepted_moves
        ));
    }
    if captured.life_totals[1] > 0 {
        return Err(format!("opponent life did not reach zero: {:?}", captured.life_totals));
    }
    let relevant: Vec<String> = events
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            *index == casts[0]
                || *index == casts[1]
                || *index == dredge_index
                || *index == block_index
                || matches!(event, GameEvent::SpellResolved { card } if *card == mossdog)
                || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == mossdog)
                || matches!(event, GameEvent::GameEnded { .. })
        })
        .map(|(index, event)| format!("{index}: {event:?}"))
        .collect();
    Ok(json!({
        "trace_id": "rav-greater-mossdog-dredge-recast-full-game",
        "deck_p0": "4 Greater Mossdog + 56 Forest (60 shuffled cards)",
        "deck_p1": "1 Watchwolf + 29 Forest + 30 Plains (60 shuffled cards)",
        "shuffle_seed": seed,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "rejected_moves": 0,
        "life_totals": captured.life_totals,
        "mossdog_object": format!("{mossdog:?}"),
        "watchwolf_object": format!("{watchwolf:?}"),
        "cast_payments": payments.iter().take(2).map(|colors| colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "dredge_count": 3,
        "mossdog_power": captured.mossdog_pt.map(|pt| pt.0),
        "mossdog_toughness": captured.mossdog_pt.map(|pt| pt.1),
        "mossdog_zone": format!("{:?}", captured.mossdog_zone),
        "recast_combat_damage": combat_damage,
        "mill_object_ids": mills.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>(),
        "event_count": events.len(),
        "event_digest": format!("fnv1a64:{:016x}", event_digest(events)),
        "events": relevant
    }))
}

fn validate_dredge_suffix(
    events: &[GameEvent],
    dredge_index: usize,
    mossdog: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    if dredge_index < 8 {
        return Err("Dredge event has too little preceding zone provenance".to_owned());
    }
    let suffix = &events[dredge_index - 8..=dredge_index];
    let mut mills = Vec::new();
    for pair in suffix[..6].chunks_exact(2) {
        match pair {
            [GameEvent::CardMoved { card, to: Zone::Graveyard }, GameEvent::ObjectIncarnationAdvanced { object, incarnation: _ }]
                if card == object => mills.push(*card),
            _ => return Err(format!("Dredge did not mill three exact objects in order: {pair:?}")),
        }
    }
    match &suffix[6..] {
        [
            GameEvent::CardMoved { card, to: Zone::Hand },
            GameEvent::ObjectIncarnationAdvanced { object, incarnation: _ },
            GameEvent::Dredged { player: PlayerId(0), card: dredged, count: 3 },
        ] if *card == mossdog && *object == mossdog && *dredged == mossdog => {}
        other => return Err(format!("Dredge did not return the same object to hand: {other:?}")),
    }
    if mills.len() != 3 || mills.contains(&mossdog) || mills.iter().copied().collect::<BTreeSet<_>>().len() != 3 {
        return Err(format!("Dredge mill objects are not three distinct cards: {mills:?}"));
    }
    Ok(mills)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Greater Mossdog source: {error}"))?;
    if source.len() > 32 * 1024
        || source.contains("CreatureBecomesTarget")
        || source.contains("targeted-creature-return-from-graveyard")
        || source.contains("ReturnSourceFromOwnersGraveyardToHand")
        || source.contains("triggered_ability")
    {
        return Err("Greater Mossdog source contains the unprinted target trigger or exceeds its source boundary".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Greater Mossdog source is not valid Rust".to_owned())?;
    if file.items.len() != 3 || !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) {
        return Err("Greater Mossdog module must contain only its docs, imports, and definition".to_owned());
    }
    let mut definition_found = false;
    for item in &file.items {
        match item {
            Item::Use(_) => {}
            Item::Fn(function) if function.sig.ident == "definition" && !definition_found => {
                definition_found = true;
                if !function.sig.inputs.is_empty()
                    || !matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(structure), None) if structure.path.is_ident("CardDefinition"))
                {
                    return Err("Greater Mossdog definition must be one crate-visible literal".to_owned());
                }
            }
            _ => return Err("Greater Mossdog module contains an unexpected top-level item".to_owned()),
        }
    }
    if !definition_found {
        return Err("Greater Mossdog definition function is missing".to_owned());
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_greater_mossdog::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || (definition.power, definition.toughness) != (Some(3), Some(3))
        || definition.keywords != [Keyword::Dredge(3)]
        || !definition.effects.is_empty()
        || definition.supported_rules.contains(&"targeted-creature-return-from-graveyard")
    {
        return Err("Greater Mossdog source does not match the printed Dredge 3-only contract".to_owned());
    }
    Ok(())
}

fn validate_registry() -> Result<(), String> {
    if rav_triggered_ability_bindings()
        .iter()
        .any(|binding| binding.card_definition == CARD_ID)
    {
        return Err("Greater Mossdog has an unprinted triggered ability binding".to_owned());
    }
    Ok(())
}

fn target_trigger_mutation_detected() -> Result<bool, String> {
    let mut triggers = rav_triggered_ability_bindings();
    triggers.push(TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: "targeted-creature-return-from-graveyard",
            condition: TriggerCondition::CreatureBecomesTarget,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::ReturnSourceFromOwnersGraveyardToHand],
        },
    });
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;
    let mossdog = game
        .add_card(PlayerId(0), CARD_ID, Zone::Graveyard)
        .map_err(|error| error.to_string())?;
    let guildmage = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-GUILDMAGE")
        .map_err(|error| error.to_string())?;
    let target = game
        .put_on_battlefield(PlayerId(1), OPPONENT_ID)
        .map_err(|error| error.to_string())?;
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .map_err(|error| error.to_string())?;
    for permanent in [guildmage, target, mountain] {
        game.set_entered_turn_for_setup(permanent, 0)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    while game.step != Step::PrecombatMain {
        let player = game.priority;
        game.pass_priority(player).map_err(|error| error.to_string())?;
    }
    game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
        .map_err(|error| error.to_string())?;
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guildmage,
            ability_id: "grant-haste",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == mossdog && *ability == "targeted-creature-return-from-graveyard")
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
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
        "qualification": if passed {"diagnostic_trace_passed; independent review pending"} else {"not_evaluated"},
        "trace_status": if passed {"passed"} else {"not_qualified"},
        "credited": false
    })
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

fn event_digest(events: &[GameEvent]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{events:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x00000100000001b3);
    }
    hash
}
