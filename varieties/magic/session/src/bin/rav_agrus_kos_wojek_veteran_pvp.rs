//! Isolated full-game RAV #190 Agrus Kos, Wojek Veteran authority.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ObjectId, PlayerId, PolicyAction, Step, TriggerCondition, TriggeredAbilityBinding,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-AGRUS-KOS-WOJEK-VETERAN";
const CARD_NAME: &str = "Agrus Kos, Wojek Veteran";
const TRIGGER_ID: &str = "attack-color-specific-combat-modifiers";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/agrus_kos_wojek_veteran.rs";
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/agrus_kos_wojek_veteran.rs"]
mod candidate_agrus_kos;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Agrus Kos PvP result: {error}");
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
            eprintln!("Agrus Kos PvP authority failed: {error}");
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
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Agrus Kos shuffled PvP replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "wrong_mana_cost_detected": mutation_detected(seed, Mutation::WrongManaCost),
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower),
        "wrong_toughness_detected": mutation_detected(seed, Mutation::WrongToughness),
        "missing_trigger_detected": mutation_detected(seed, Mutation::MissingTrigger),
        "wrong_trigger_condition_detected": mutation_detected(seed, Mutation::WrongTriggerCondition),
        "red_modifier_mutation_detected": mutation_detected(seed, Mutation::WrongRedModifier),
        "white_modifier_mutation_detected": mutation_detected(seed, Mutation::WrongWhiteModifier),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "missing_trigger_detected",
        "wrong_trigger_condition_detected",
        "red_modifier_mutation_detected",
        "white_modifier_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Agrus Kos semantic mutation escaped: {key}"));
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
        "card": {"collector_number": 190, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and stack resolution",
                "202.1-202.2: colors and mana cost",
                "208.1: power and toughness",
                "508.1: declaring attackers",
                "603.2: attack-triggered ability",
                "608.2: ability resolution",
                "613.4: power/toughness modifiers",
                "514: cleanup expiration"
            ],
            "source_contract": "RAV #190 is a legendary red-white {3}{R}{W} 3/3 Human Soldier whose attack trigger gives attacking red creatures +2/+0 and attacking white creatures +0/+2 until end of turn",
            "source_ruling": "The 2005 Ravnica FAQ confirms a red-white attacker, including Agrus, receives both independent bonuses",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
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
        "candidate_source_validation": {
            "normalized_literal_source_checked_before_candidate_calls": true,
            "direct_literal_ast_checked_before_candidate_calls": true
        },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    WrongPower,
    WrongToughness,
    MissingTrigger,
    WrongTriggerCondition,
    WrongRedModifier,
    WrongWhiteModifier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    agrus: Option<ObjectId>,
    witness_ids: BTreeMap<&'static str, ObjectId>,
    before_pt: BTreeMap<&'static str, (i16, i16)>,
    after_pt: BTreeMap<&'static str, (i16, i16)>,
    cleanup_pt: BTreeMap<&'static str, (i16, i16)>,
    cast_payment: Vec<Color>,
    trigger_stack_index: Option<usize>,
    trigger_resolved_index: Option<usize>,
    cleanup_expirations: usize,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    for seed in 0..=128 {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err("no shuffled Agrus Kos full-game trace found for seeds 0..=128".to_owned())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_agrus_kos::definition();
    let mut trigger = candidate_agrus_kos::triggered_ability();
    match mutation {
        Mutation::None | Mutation::MissingTrigger => {}
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Red, Color::White]);
        }
        Mutation::WrongPower => definition.power = Some(4),
        Mutation::WrongToughness => definition.toughness = Some(4),
        Mutation::WrongTriggerCondition => {
            trigger.ability.condition = TriggerCondition::CastsCreatureSpell
        }
        Mutation::WrongRedModifier => {
            if let Some(Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn { power, .. }) =
                trigger.ability.effects.iter_mut().find(|effect| {
                    matches!(
                        effect,
                        Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                            color: Color::Red,
                            ..
                        }
                    )
                })
            {
                *power = 1;
            }
        }
        Mutation::WrongWhiteModifier => {
            if let Some(Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                toughness, ..
            }) = trigger.ability.effects.iter_mut().find(|effect| {
                matches!(
                    effect,
                    Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                        color: Color::White,
                        ..
                    }
                )
            }) {
                *toughness = 1;
            }
        }
    }
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Agrus Kos is missing from the RAV definition registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Agrus Kos trigger is missing from the RAV trigger registry")?;
    if mutation == Mutation::MissingTrigger {
        triggers.remove(trigger_index);
    } else {
        triggers[trigger_index] = trigger;
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        cardbench_magic_rav::rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;

    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-BOROS-RECRUIT".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-GOBLIN-SPELUNKERS".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-WATCHWOLF".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 15,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 15,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 14,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let count = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
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
        Box::new(AgrusPolicy::new(PlayerId(0), true)),
        Box::new(AgrusPolicy::new(PlayerId(1), false)),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        agrus: None,
        witness_ids: BTreeMap::new(),
        before_pt: BTreeMap::new(),
        after_pt: BTreeMap::new(),
        cleanup_pt: BTreeMap::new(),
        cast_payment: vec![],
        trigger_stack_index: None,
        trigger_resolved_index: None,
        cleanup_expirations: 0,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Agrus Kos match exceeded its bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && !view.attackers_declared
            && captured.witness_ids.is_empty()
        {
            if let Some(witnesses) = select_witnesses(&view) {
                captured.witness_ids = witnesses;
                captured.agrus = captured.witness_ids.get("agrus").copied();
                captured.before_pt = snapshot_witnesses(&game, &captured.witness_ids)?;
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Agrus Kos policy seat missing")?;
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
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Agrus Kos policy action rejected on turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let new_events = game.event_log[captured.events.len()..].to_vec();
        observe(&game, &new_events, &mut captured)?;
        captured.events.extend(new_events);
    }
    captured.winner = game.winner().map(|player| player.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    Ok(captured)
}

fn select_witnesses(view: &GameView) -> Option<BTreeMap<&'static str, ObjectId>> {
    let agrus = view
        .own_battlefield
        .iter()
        .find(|card| card.definition == Some(CARD_ID) && card.can_attack)?;
    let boros = view
        .own_battlefield
        .iter()
        .find(|card| card.definition == Some("RAV-BOROS-RECRUIT") && card.can_attack)?;
    let white = view
        .own_battlefield
        .iter()
        .find(|card| card.definition == Some("RAV-WATCHWOLF") && card.can_attack)?;
    let reds = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-GOBLIN-SPELUNKERS") && card.can_attack)
        .collect::<Vec<_>>();
    let attacking_red = *reds.first()?;
    let nonattacking_red = *reds.get(1)?;
    Some(BTreeMap::from([
        ("agrus", agrus.id),
        ("red_white", boros.id),
        ("red", attacking_red.id),
        ("white", white.id),
        ("nonattacking_red", nonattacking_red.id),
    ]))
}

fn snapshot_witnesses(
    game: &Game,
    witnesses: &BTreeMap<&'static str, ObjectId>,
) -> Result<BTreeMap<&'static str, (i16, i16)>, String> {
    witnesses
        .iter()
        .map(|(label, object)| {
            let characteristics = game
                .characteristics(*object)
                .map_err(|error| error.to_string())?;
            let power = characteristics
                .power
                .ok_or("witness creature has no power")?;
            let toughness = characteristics
                .toughness
                .ok_or("witness creature has no toughness")?;
            Ok((*label, (power, toughness)))
        })
        .collect()
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut CapturedMatch) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.cast_payment.clone_from(colors);
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.agrus = Some(*card);
            }
            GameEvent::TriggeredAbilityStacked {
                source, ability, ..
            } if *ability == TRIGGER_ID && captured.trigger_stack_index.is_none() => {
                if captured.agrus == Some(*source) {
                    captured.trigger_stack_index = Some(
                        captured.events.len()
                            + events
                                .iter()
                                .position(|candidate| std::ptr::eq(candidate, event))
                                .unwrap_or(0),
                    );
                }
            }
            GameEvent::AbilityResolved {
                source, ability, ..
            } if *ability == TRIGGER_ID
                && captured.agrus == Some(*source)
                && captured.after_pt.is_empty() =>
            {
                captured.trigger_resolved_index = Some(
                    captured.events.len()
                        + events
                            .iter()
                            .position(|candidate| std::ptr::eq(candidate, event))
                            .unwrap_or(0),
                );
                captured.after_pt = snapshot_witnesses(game, &captured.witness_ids)?;
            }
            GameEvent::ContinuousEffectExpired {
                source,
                layer: cardbench_magic_engine::Layer::PowerToughness,
                ..
            } if captured.agrus == Some(*source) => {
                captured.cleanup_expirations += 1;
                if captured.cleanup_pt.is_empty() && captured.cleanup_expirations >= 6 {
                    captured.cleanup_pt = snapshot_witnesses(game, &captured.witness_ids)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct AgrusPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    trigger_witness_attack_done: bool,
}

impl AgrusPolicy {
    fn new(player: PlayerId, pilot: bool) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            trigger_witness_attack_done: false,
        }
    }
}

impl CodePolicy for AgrusPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.agrus-kos-cohort-pilot.v1"
        } else {
            "rav.agrus-kos-land-only-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.step == Step::DeclareBlockers
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if self.pilot
            && view.player == self.player
            && view.step == Step::DeclareAttackers
            && !view.attackers_declared
        {
            if !self.trigger_witness_attack_done {
                if let Some(witnesses) = select_witnesses(view) {
                    self.trigger_witness_attack_done = true;
                    return PolicyAction::DeclareAttackers {
                        attackers: ["agrus", "red_white", "red", "white"]
                            .into_iter()
                            .filter_map(|label| witnesses.get(label).copied())
                            .collect(),
                    };
                }
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.definition != Some(CARD_ID))
                    .map(|card| card.id)
                    .collect(),
            };
        }
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err("Agrus Kos duel did not reach a clean seat-zero terminal win".to_owned());
    }
    if captured.accepted_moves == 0 || captured.turns == 0 || captured.events.is_empty() {
        return Err("Agrus Kos full-game trace is empty".to_owned());
    }
    let witnesses = &captured.witness_ids;
    if witnesses.len() != 5 || captured.agrus != witnesses.get("agrus").copied() {
        return Err("the full red/white attacker cohort did not assemble".to_owned());
    }
    if captured.cast_payment.len() != 5
        || !captured.cast_payment.contains(&Color::Red)
        || !captured.cast_payment.contains(&Color::White)
    {
        return Err(format!(
            "Agrus Kos cast did not pay {{3}}{{R}}{{W}}: {:?}",
            captured.cast_payment
        ));
    }
    let before = &captured.before_pt;
    let after = &captured.after_pt;
    let base = [
        ("agrus", (3, 3)),
        ("red_white", (1, 1)),
        ("red", (2, 2)),
        ("white", (3, 3)),
        ("nonattacking_red", (2, 2)),
    ];
    let boosted = [
        ("agrus", (5, 5)),
        ("red_white", (3, 3)),
        ("red", (4, 2)),
        ("white", (3, 5)),
        ("nonattacking_red", (2, 2)),
    ];
    for (label, expected) in base {
        if before.get(label) != Some(&expected) {
            return Err(format!(
                "{label} base characteristics differ: {:?}",
                before.get(label)
            ));
        }
    }
    for (label, expected) in boosted {
        if after.get(label) != Some(&expected) {
            return Err(format!(
                "Agrus attack modifiers produced the wrong {label} power/toughness: {:?}",
                after.get(label)
            ));
        }
    }
    let stack = captured
        .trigger_stack_index
        .ok_or("Agrus attack trigger was not stacked")?;
    let resolved = captured
        .trigger_resolved_index
        .ok_or("Agrus attack trigger did not resolve")?;
    if stack >= resolved {
        return Err("Agrus attack trigger stack/resolution order differs".to_owned());
    }
    if captured.cleanup_expirations != 6 || captured.cleanup_pt != captured.before_pt {
        return Err(format!(
            "Agrus temporary modifiers failed to expire at Cleanup ({} effect receipts)",
            captured.cleanup_expirations
        ));
    }
    if !captured.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    }) {
        return Err("seat-zero GameEnded receipt is missing".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-agrus-kos-color-specific-attacker-modifiers-full-game",
        "passed": true,
        "seed": seed,
        "terminal": captured.terminal,
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "deck_p0": "4 Agrus Kos + 4 Boros Recruit + 4 Goblin Spelunkers + 4 Watchwolf + 44 RAV basic lands (60 shuffled cards)",
        "deck_p1": "60 Plains (60 shuffled cards)",
        "mana_paid": captured.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "cohort_base_power_toughness": before.iter().map(|(label, pt)| (label.to_string(), json!([pt.0, pt.1]))).collect::<BTreeMap<_,_>>(),
        "cohort_after_trigger_power_toughness": after.iter().map(|(label, pt)| (label.to_string(), json!([pt.0, pt.1]))).collect::<BTreeMap<_,_>>(),
        "cleanup_power_toughness": captured.cleanup_pt.iter().map(|(label, pt)| (label.to_string(), json!([pt.0, pt.1]))).collect::<BTreeMap<_,_>>(),
        "trigger_stack_index": stack,
        "trigger_resolution_index": resolved,
        "cleanup_modifier_expirations": captured.cleanup_expirations,
        "events": project_events(&captured.events),
        "event_count": captured.events.len(),
        "ordered_event_sequence": [stack, resolved],
        "mechanics": {
            "definition_id": CARD_ID,
            "mana_cost": "{3}{R}{W}",
            "trigger_id": TRIGGER_ID,
            "red_modifier": [2,0],
            "white_modifier": [0,2],
            "red_white_attacker_gains_both": true,
            "nonattacking_red_unchanged": true,
            "all_modifiers_expire_at_cleanup": true
        }
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Agrus Kos source: {error}"))?;
    if source.len() > 32 * 1024
        || source.contains("include!")
        || source.contains("std::fs")
        || source.contains("std::process")
    {
        return Err("Agrus Kos source exceeds the literal module boundary".to_owned());
    }
    let _file =
        syn::parse_file(&source).map_err(|_| "Agrus Kos source is not valid Rust".to_owned())?;
    if source.trim() != EXPECTED_SOURCE.trim() {
        return Err("Agrus Kos source differs from the pinned literal card module".to_owned());
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_agrus_kos::definition();
    let trigger = candidate_agrus_kos::triggered_ability();
    let effects = [
        Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
            color: Color::Red,
            power: 2,
            toughness: 0,
        },
        Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
            color: Color::White,
            power: 0,
            toughness: 2,
        },
    ];
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Red, Color::White])
        || definition.colors != BTreeSet::from([Color::Red, Color::White])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || (definition.power, definition.toughness) != (Some(3), Some(3))
        || trigger.card_definition != CARD_ID
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition != TriggerCondition::Attacks
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != effects
    {
        return Err("candidate definition or trigger differs from RAV #190".to_owned());
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect::<Vec<_>>();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
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
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot trace passed; independent review pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
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

// Pinned canonical candidate text; candidate evaluation is refused before the
// module's builder functions run if this exact literal program has changed.
const EXPECTED_SOURCE: &str = r#"//! Candidate-owned RAV Agrus Kos, Wojek Veteran definition and attack trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

/// RAV #190: a legendary red-white 3/3 Human Soldier for `{3}{R}{W}`.
/// Legendary-supertype metadata is supplied by the set binding registry.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-AGRUS-KOS-WOJEK-VETERAN",
        name: "Agrus Kos, Wojek Veteran",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "attack-triggered-color-specific-combat-modifiers",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// When Agrus attacks, each attacking red creature gets +2/+0 and each
/// attacking white creature gets +0/+2 until end of turn. A red-white
/// attacker, including Agrus, receives both independent effects.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-AGRUS-KOS-WOJEK-VETERAN",
        ability: TriggeredAbility {
            id: "attack-color-specific-combat-modifiers",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                    color: Color::Red,
                    power: 2,
                    toughness: 0,
                },
                Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                    color: Color::White,
                    power: 0,
                    toughness: 2,
                },
            ],
        },
    }
}"#;
