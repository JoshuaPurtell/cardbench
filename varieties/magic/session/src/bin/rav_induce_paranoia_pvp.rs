//! Full-game RAV Induce Paranoia authority for countering a spell and the
//! black-mana-conditional mill of that spell's controller.

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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-INDUCE-PARANOIA";
const CARD_NAME: &str = "Induce Paranoia";
const TARGET_ID: &str = "RAV-CARVEN-CARYATID";
const PILOT_THREAT_ID: &str = "RAV-SNAPPING-DRAKE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/induce_paranoia.rs";
fn induce_cost() -> ManaCost {
    ManaCost::with_colors(2, [Color::Blue, Color::Blue])
}

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/induce_paranoia.rs"]
mod candidate_induce_paranoia;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Induce Paranoia PvP result: {error}");
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
            eprintln!("Induce Paranoia PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // Candidate source validation must remain the first operation that could
    // lead to a candidate factory call.
    validate_candidate_source()?;
    let source_guard = validate_compile_valid_side_effect_candidate()?;
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
        || !printings
            .iter()
            .any(|card| card.collector_number == 56 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }

    let (black_seed, black_match) = find_trace(Scenario::BlackSpent)?;
    let black_trace = validate_trace(&black_match, black_seed, Scenario::BlackSpent)?;
    if !same_match(
        &black_match,
        &run_match(black_seed, Scenario::BlackSpent, Mutation::None)?,
    ) {
        return Err("black-spent full-game replay was not deterministic".to_owned());
    }
    let (plain_seed, plain_match) = find_trace(Scenario::NoBlack)?;
    let plain_trace = validate_trace(&plain_match, plain_seed, Scenario::NoBlack)?;
    if !same_match(
        &plain_match,
        &run_match(plain_seed, Scenario::NoBlack, Mutation::None)?,
    ) {
        return Err("no-black full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "unconditional_counter_without_conditional_mill_detected_in_game": mutation_detected(black_seed, Mutation::NoConditionalMill),
        "wrong_mana_color_condition_detected_in_game": mutation_detected(black_seed, Mutation::WrongConditionColor),
        "wrong_spell_cost_detected_in_game": mutation_detected(black_seed, Mutation::WrongCost),
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "unconditional_counter_without_conditional_mill_detected_in_game",
        "wrong_mana_color_condition_detected_in_game",
        "wrong_spell_cost_detected_in_game",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Induce Paranoia semantic mutation escaped: {key}"));
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
        "card": {"collector_number": 56, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: spell color and colored mana cost",
                "106 and 601.2: generic payment and casting a spell with explicit mana choices",
                "117: instant-speed priority response window",
                "608.2b-608.3: target legality and spell resolution",
                "701.5: countering a physical spell",
                "701.13: milling the target spell controller",
                "202.3: mana value of the target spell",
                "510: combat damage and a terminal two-player result"
            ],
            "source_contract": "Original RAV #56 printing: {2}{U}{U} blue instant; counter target spell; if black mana was spent to cast it, that spell's controller puts the top X cards of their library into their graveyard, where X is that spell's converted mana cost.",
            "oracle_reconciliation": "Current Oracle replaces the original 'puts the top X cards ... into the graveyard' and 'converted mana cost' phrasing with 'mills X cards' and 'mana value'. The same target spell controls both X and the library, and the black-spent condition is unchanged.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "source_guard": source_guard,
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
        "qualification_status": "diagnostic_only; independent review and candidate trial remain",
        "mutation_sensitivity": mutations,
        "traces": [black_trace, plain_trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    BlackSpent,
    NoBlack,
}

impl Scenario {
    fn branch(self) -> &'static str {
        match self {
            Self::BlackSpent => "black_spent",
            Self::NoBlack => "no_black",
        }
    }

    fn trace_id(self) -> &'static str {
        match self {
            Self::BlackSpent => "rav-induce-paranoia-counter-and-mill",
            Self::NoBlack => "rav-induce-paranoia-counter-without-mill-control",
        }
    }

    fn spends_black(self) -> bool {
        self == Self::BlackSpent
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConditionalMill,
    WrongConditionColor,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    induce: Option<ObjectId>,
    target: Option<ObjectId>,
    pilot_threat: Option<ObjectId>,
    induce_payment: Vec<Color>,
    target_payment: Vec<Color>,
    expected_library_top: Vec<ObjectId>,
    zones: [Option<Zone>; 3],
    life: [i64; 2],
}

fn find_trace(scenario: Scenario) -> Result<(u64, Captured), String> {
    let env_key = match scenario {
        Scenario::BlackSpent => "CARDBENCH_INDUCE_PARANOIA_BLACK_SEED",
        Scenario::NoBlack => "CARDBENCH_INDUCE_PARANOIA_NO_BLACK_SEED",
    };
    if let Ok(value) = std::env::var(env_key) {
        let seed = value
            .parse::<u64>()
            .map_err(|_| format!("{env_key} must be an unsigned integer"))?;
        let captured = run_match(seed, scenario, Mutation::None)?;
        validate_trace(&captured, seed, scenario)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=512 {
        match run_match(seed, scenario, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed, scenario).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no shuffled game produced the complete Induce Paranoia {scenario:?} trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, Scenario::BlackSpent, mutation)
        .and_then(|captured| validate_trace(&captured, seed, Scenario::BlackSpent).map(|_| ()))
        .is_err()
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_induce_paranoia::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConditionalMill => definition.effects = vec![Effect::CounterTargetSpell],
        Mutation::WrongConditionColor => {
            definition.effects = vec![
                Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                    color: Color::Green,
                },
            ];
        }
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
        }
    }
    definition
}

fn run_match(seed: u64, scenario: Scenario, mutation: Mutation) -> Result<Captured, String> {
    let pilot_lands = if scenario.spends_black() {
        vec![
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 36,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 16,
            },
        ]
    } else {
        vec![DeckEntry {
            card: "RAV-ISLAND".to_owned(),
            count: 52,
        }]
    };
    let mut pilot_cards = vec![
        DeckEntry {
            card: CARD_ID.to_owned(),
            count: 4,
        },
        DeckEntry {
            card: PILOT_THREAT_ID.to_owned(),
            count: 4,
        },
    ];
    pilot_cards.extend(pilot_lands);
    let pilot_deck = DeckList {
        mainboard: pilot_cards,
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{label} deck has {size} cards; expected 60"));
        }
    }

    let mut definitions = card_definitions();
    let submitted = mutated_definition(mutation);
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Induce Paranoia is absent from the RAV registry")? = submitted;
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
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
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

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(InducePolicy::new(PlayerId(0), scenario)),
        Box::new(InducePolicy::new(PlayerId(1), scenario)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut expected_library_top = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(64) || accepted >= config.max_policy_moves {
            return Err("Induce Paranoia match exceeded its bounded turn or move limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let target_is_on_stack = view
            .stack_spells
            .iter()
            .any(|spell| spell.definition == Some(TARGET_ID));
        if player == PlayerId(0) && target_is_on_stack && expected_library_top.is_empty() {
            expected_library_top = game
                .player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .library
                .iter()
                .rev()
                .take(3)
                .copied()
                .collect();
        }
        let policy = policies.get_mut(player.0).ok_or("missing seat policy")?;
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
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }

    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, definition_id: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == definition_id) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let induce = find_cast(PlayerId(0), CARD_ID);
    let target = find_cast(PlayerId(1), TARGET_ID);
    let pilot_threat = find_cast(PlayerId(0), PILOT_THREAT_ID);
    let payment = |seat: PlayerId, card: Option<ObjectId>| {
        card.and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player,
                    card: paid_card,
                    colors,
                } if *player == seat && *paid_card == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
    };
    let end_index = events
        .iter()
        .position(|event| matches!(event, GameEvent::GameEnded { .. }))
        .unwrap_or(events.len());
    let zone_at_end = |card: Option<ObjectId>| {
        card.and_then(|card| {
            events[..end_index]
                .iter()
                .rev()
                .find_map(|event| match event {
                    GameEvent::CardMoved {
                        card: moved, to, ..
                    } if *moved == card => Some(*to),
                    _ => None,
                })
        })
    };
    Ok(Captured {
        events,
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        induce,
        target,
        pilot_threat,
        induce_payment: payment(PlayerId(0), induce),
        target_payment: payment(PlayerId(1), target),
        expected_library_top,
        zones: [
            zone_at_end(induce),
            zone_at_end(target),
            zone_at_end(pilot_threat),
        ],
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

struct InducePolicy {
    player: PlayerId,
    scenario: Scenario,
    target_cast: bool,
    induce_submitted: bool,
    threat_submitted: bool,
}

impl InducePolicy {
    fn new(player: PlayerId, scenario: Scenario) -> Self {
        Self {
            player,
            scenario,
            target_cast: false,
            induce_submitted: false,
            threat_submitted: false,
        }
    }
}

impl CodePolicy for InducePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.induce-paranoia-counter-mill-pilot.v1"
        } else {
            "rav.induce-paranoia-carven-caryatid-opponent.v1"
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
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
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
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return self.respond_or_pass(view);
        }

        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }

        if self.player == PlayerId(1) && !self.target_cast {
            let opponent_lands: Vec<_> = view
                .opponent_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
                .collect();
            let open_blue = opponent_lands
                .iter()
                .filter(|card| card.mana_colors.contains(&Color::Blue))
                .count();
            let open_black = opponent_lands
                .iter()
                .any(|card| card.mana_colors.contains(&Color::Black));
            let target_is_ready = opponent_lands.len() >= 4
                && open_blue >= 2
                && (!self.scenario.spends_black() || open_black);
            if target_is_ready
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
            {
                let cost = ManaCost::with_colors(1, [Color::Green, Color::Green]);
                if let Some(selection) = payment_selection(view, &cost, false) {
                    self.target_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost, false) {
                    return action;
                }
            }
        }

        if self.player == PlayerId(0) && self.induce_submitted && !self.threat_submitted {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(PILOT_THREAT_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Blue]);
                if let Some(selection) = payment_selection(view, &cost, false) {
                    self.threat_submitted = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost, false) {
                    return action;
                }
            }
        }
        self.respond_or_pass(view)
    }
}

impl InducePolicy {
    fn respond_or_pass(&mut self, view: &GameView) -> PolicyAction {
        if self.player == PlayerId(0)
            && !self.induce_submitted
            && let Some(target) = view
                .stack_spells
                .iter()
                .find(|spell| spell.definition == Some(TARGET_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            if let Some(selection) =
                payment_selection(view, &induce_cost(), self.scenario.spends_black())
            {
                self.induce_submitted = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![Target::Spell(target.id)],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &induce_cost(), self.scenario.spends_black()) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment_selection(
    view: &GameView,
    cost: &ManaCost,
    spend_black: bool,
) -> Option<ManaPaymentSelection> {
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
    let mut black_reserved = spend_black;
    for _ in 0..cost.generic {
        let preferred = if black_reserved {
            black_reserved = false;
            Some(Color::Black)
        } else {
            None
        };
        let chosen = preferred
            .filter(|color| {
                available
                    .iter()
                    .any(|(known, amount)| known == color && *amount > 0)
            })
            .or_else(|| {
                [
                    Color::Blue,
                    Color::Green,
                    Color::White,
                    Color::Red,
                    Color::Black,
                ]
                .into_iter()
                .find(|color| {
                    available
                        .iter()
                        .any(|(known, amount)| known == color && *amount > 0)
                })
            })?;
        let (_, amount) = available.iter_mut().find(|(known, _)| *known == chosen)?;
        *amount -= 1;
        generic.push(chosen);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn make_mana(view: &GameView, cost: &ManaCost, spend_black: bool) -> Option<PolicyAction> {
    let required_blue = cost
        .colored
        .iter()
        .filter(|color| **color == Color::Blue)
        .count() as u8;
    let missing_blue = required_blue.saturating_sub(view.mana_pool.amount(Color::Blue));
    let needed = if missing_blue > 0 {
        Some(Color::Blue)
    } else if spend_black && view.mana_pool.amount(Color::Black) == 0 {
        Some(Color::Black)
    } else if view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8) {
        Some(Color::Blue)
    } else {
        None
    }?;
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

fn validate_trace(captured: &Captured, seed: u64, scenario: Scenario) -> Result<Value, String> {
    let induce = captured.induce.ok_or("Induce Paranoia was not cast")?;
    let target = captured.target.ok_or("the target spell was not cast")?;
    let threat = captured
        .pilot_threat
        .ok_or("pilot never cast its terminal threat")?;
    if captured.expected_library_top.len() != 3 {
        return Err(
            "authority failed to snapshot three top library objects before the response".to_owned(),
        );
    }
    if captured.zones
        != [
            Some(Zone::Graveyard),
            Some(Zone::Graveyard),
            Some(Zone::Battlefield),
        ]
    {
        return Err(format!(
            "countered spell, Induce Paranoia, and pilot threat zones differ: {:?}",
            captured.zones
        ));
    }
    if captured.induce_payment.len() != 4
        || captured
            .induce_payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            != 2
        || (scenario.spends_black()
            && captured
                .induce_payment
                .iter()
                .filter(|color| **color == Color::Black)
                .count()
                != 1)
        || (!scenario.spends_black() && captured.induce_payment.contains(&Color::Black))
    {
        return Err(format!(
            "Induce Paranoia was not paid as {2}{U}{U} with the requested black-spend branch: {:?}",
            captured.induce_payment
        ));
    }
    if captured.target_payment.len() != 3
        || captured
            .target_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            < 2
    {
        return Err(format!(
            "Carven Caryatid target spell did not pay {1}{G}{G}: {:?}",
            captured.target_payment
        ));
    }
    let events = &captured.events;
    let target_paid = event_index(events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(1), card, colors }
            if *card == target && colors.len() == 3 && colors.iter().filter(|color| **color == Color::Green).count() >= 2
    )).ok_or("target spell payment receipt is missing")?;
    let target_cast = event_index_after(events, target_paid, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(1), card } if *card == target
        )
    })
    .ok_or("target spell cast receipt is missing")?;
    let induce_paid = event_index_after(events, target_cast, |event| {
        matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                if *card == induce && colors == &captured.induce_payment
        )
    })
    .ok_or("Induce Paranoia payment receipt does not follow the target cast")?;
    let induce_cast = event_index_after(events, induce_paid, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == induce
        )
    })
    .ok_or("Induce Paranoia response cast receipt is missing")?;
    let target_was_named = events[..induce_cast].iter().any(|event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(1), card } if *card == target
        )
    });
    if !target_was_named {
        return Err(
            "Induce Paranoia did not respond to the exact spell already on the stack".to_owned(),
        );
    }
    let (counter, target_move) =
        validate_counter_target_cleanup(events, induce, target, induce_cast)?;
    let source_resolved = event_index_after(events, induce_cast, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == induce
        )
    })
    .ok_or("Induce Paranoia did not resolve")?;
    let mill_indices: Vec<usize> = captured
        .expected_library_top
        .iter()
        .filter_map(|expected| {
            events
                .iter()
                .enumerate()
                .skip(target_move + 1)
                .take(source_resolved.saturating_sub(target_move + 1))
                .find_map(|(index, event)| {
                    matches!(event,
                        GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if card == expected
                    )
                    .then_some(index)
                })
        })
        .collect();
    if scenario.spends_black() {
        if mill_indices.len() != 3 || mill_indices.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(format!(
                "black-spent Induce Paranoia did not mill the exact three top objects in order: expected={:?}, indices={mill_indices:?}",
                captured.expected_library_top
            ));
        }
        for (index, object) in mill_indices.iter().zip(&captured.expected_library_top) {
            if !matches!(events.get(index + 1), Some(GameEvent::ObjectIncarnationAdvanced { object: advanced, .. }) if advanced == object)
            {
                return Err(format!(
                    "mill movement for {object:?} lacks its adjacent incarnation receipt"
                ));
            }
        }
    } else if !mill_indices.is_empty() {
        return Err(format!(
            "no-black Induce Paranoia unexpectedly milled an opponent library card: {mill_indices:?}"
        ));
    }
    if !(target_paid < target_cast
        && target_cast < induce_paid
        && induce_paid < induce_cast
        && induce_cast < counter
        && counter < target_move
        && target_move < source_resolved)
    {
        return Err("target cast, instant response, counter, mill branch, and source resolution order differs".to_owned());
    }
    if captured.winner != Some(0)
        || captured.life[0] <= 0
        || captured.life[1] > 0
        || captured.rejected != 0
        || captured.turns > 64
    {
        return Err(format!(
            "full-game trace did not reach a bounded clean pilot win: winner={:?}, life={:?}, rejected={}",
            captured.winner, captured.life, captured.rejected
        ));
    }
    let attack = event_index(events, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&threat)
    )).ok_or("pilot's Snapping Drake did not attack")?;
    let damage = event_index_after(events, attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == threat && *amount > 0
    )).ok_or("pilot's Snapping Drake dealt no combat damage")?;
    let ended = event_index_after(events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win event is missing")?;
    let ordered = if scenario.spends_black() {
        vec![
            target_paid,
            target_cast,
            induce_paid,
            induce_cast,
            counter,
            target_move,
            mill_indices[0],
            mill_indices[1],
            mill_indices[2],
            source_resolved,
            attack,
            damage,
            ended,
        ]
    } else {
        vec![
            target_paid,
            target_cast,
            induce_paid,
            induce_cast,
            counter,
            target_move,
            source_resolved,
            attack,
            damage,
            ended,
        ]
    };
    if ordered.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "relevant event order is not strictly increasing: {ordered:?}"
        ));
    }
    let projected: Vec<Value> = project_events(events)
        .iter()
        .map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"canonical":event.canonical}))
        .collect();
    Ok(json!({
        "id": scenario.trace_id(),
        "branch": scenario.branch(),
        "passed": true,
        "seed": seed,
        "terminal": format!("Winner(PlayerId({}))", captured.winner.unwrap_or(99)),
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "life_at_match_end": captured.life,
        "induce_object": induce.0,
        "target_spell_object": target.0,
        "pilot_threat_object": threat.0,
        "target_spell_mana_value": 3,
        "induce_cast_payment": {"colors": captured.induce_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "blue_paid": captured.induce_payment.iter().filter(|color| **color == Color::Blue).count(), "black_spent": captured.induce_payment.contains(&Color::Black)},
        "target_cast_payment": {"colors": captured.target_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "target_countered_by_exact_induce_object": true,
        "target_spell_zone": format!("{:?}", captured.zones[1]),
        "induce_zone": format!("{:?}", captured.zones[0]),
        "mill_count": mill_indices.len(),
        "mill_object_ids": if scenario.spends_black() { captured.expected_library_top.iter().map(|object| object.0).collect::<Vec<_>>() } else { Vec::new() },
        "ordered_event_sequence": ordered,
        "event_count": events.len(),
        "events": projected
    }))
}

fn validate_counter_target_cleanup(
    events: &[GameEvent],
    induce: ObjectId,
    target: ObjectId,
    after: usize,
) -> Result<(usize, usize), String> {
    let counter = event_index_after(events, after, |event| {
        matches!(event,
            GameEvent::SpellCountered { card, source } if *card == target && *source == induce
        )
    })
    .ok_or("Induce Paranoia did not counter the exact physical target spell")?;
    let moved = event_index_after(events, counter, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == target
        )
    })
    .ok_or("countered target spell did not move to its owner's graveyard")?;
    Ok((counter, moved))
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

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: induce_cost(),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-spell-then-mill-controller-by-mana-value-if-black-spent",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                color: Color::Black,
            },
        ],
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_induce_paranoia::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Induce Paranoia is absent from the RAV registry")?;
    if candidate != expected_definition() || candidate != registered {
        return Err("Induce Paranoia candidate definition differs from its exact RAV source contract or registry".to_owned());
    }
    Ok(())
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed {"trace_passed; independent review and candidate trial pending"} else {"not_evaluated"},"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false})
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Induce Paranoia candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    if source.len() > 24 * 1024 {
        return Err("Induce Paranoia candidate source exceeds the boundary".to_owned());
    }
    let file = syn::parse_file(source)
        .map_err(|_| "Induce Paranoia candidate source is not valid Rust".to_owned())?;
    if !file.attrs.is_empty() || file.items.len() != 3 {
        return Err(
            "Induce Paranoia module must contain only two imports and its literal definition"
                .to_owned(),
        );
    }
    let [Item::Use(_), Item::Use(_), definition] = file.items.as_slice() else {
        return Err("Induce Paranoia module top-level shape changed".to_owned());
    };
    if !matches!(definition, Item::Fn(function)
        if function.sig.ident == "definition"
            && function.sig.inputs.is_empty()
            && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
            && function.block.stmts.len() == 1
            && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.path.is_ident("CardDefinition") && value.rest.is_none()))
    {
        return Err(
            "Induce Paranoia definition must be one direct crate-visible CardDefinition literal"
                .to_owned(),
        );
    }
    if normalized_source(source) != CANONICAL_CANDIDATE_SOURCE {
        return Err(
            "Induce Paranoia normalized source differs from its exact literal contract".to_owned(),
        );
    }
    Ok(())
}

fn normalized_source(source: &str) -> String {
    let mut normalized = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in source.chars() {
        if in_string {
            normalized.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
        } else if character == '"' {
            in_string = true;
            normalized.push(character);
        } else if !character.is_whitespace() {
            normalized.push(character);
        }
    }
    normalized
}

fn validate_compile_valid_side_effect_candidate() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Induce Paranoia candidate source: {error}"))?;
    let injected = format!(
        "{source}\nfn injected_side_effect() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-induce-paranoia-side-effect\"), b\"mutation\"); }}\n"
    );
    let compiles_by_construction = syn::parse_file(&injected).is_ok();
    let rejected = validate_candidate_source_text(&injected).is_err();
    if !compiles_by_construction || !rejected {
        return Err(
            "compile-valid side-effect candidate escaped the pre-call source gate".to_owned(),
        );
    }
    Ok(json!({
        "compile_valid_side_effect_candidate_by_construction": true,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": true,
        "candidate_factories_called_for_injected_source": false,
        "sealed_candidate_attestation": false
    }))
}

const CANONICAL_CANDIDATE_SOURCE: &str = r#"usecardbench_magic_engine::{CardDefinition,CardType,Color,Effect,ManaCost};usestd::collections::BTreeSet;#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:"RAV-INDUCE-PARANOIA",name:"Induce Paranoia",set_code:"RAV",mana_cost:ManaCost::with_colors(2,[Color::Blue,Color::Blue]),colors:BTreeSet::from([Color::Blue]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Instant]),is_basic_land:false,supported_rules:&["full-rules-fidelity","counter-spell-then-mill-controller-by-mana-value-if-black-spent",],power:None,toughness:None,keywords:vec![],effects:vec![Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent{color:Color::Black,},],}}"#;
