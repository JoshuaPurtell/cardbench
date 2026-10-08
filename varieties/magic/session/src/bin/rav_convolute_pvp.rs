//! Full-game RAV Convolute authority for its target-spell payment decision.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardDefinition, CardType, CastRequest, Color, DecisionKind,
    DecisionSelection, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, ResolutionPaymentManaAbility, Step,
    Target, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CONVOLUTE";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const PAY_TARGET_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const PILOT_THREAT_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/convolute.rs";
const DECK_P0: &str = "rav_convolute_blue_green_white_60";
const DECK_P1: &str = "rav_watchwolf_green_white_60";
const PAY_DECK_P1: &str = "rav_elves_green_black_60";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/convolute.rs"]
mod candidate_convolute;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Convolute PvP result: {error}");
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
            eprintln!("Convolute PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    let (decline_seed, declined) = find_trace(Scenario::Decline)?;
    let decline_trace = validate_trace(&declined, decline_seed, Scenario::Decline)?;
    let decline_replay = run_match(decline_seed, Scenario::Decline)?;
    if !same_match(&declined, &decline_replay) {
        return Err("Convolute decline-branch full-game replay was not deterministic".to_owned());
    }
    let (pay_seed, paid) = find_trace(Scenario::Pay)?;
    let pay_trace = validate_trace(&paid, pay_seed, Scenario::Pay)?;
    let pay_replay = run_match(pay_seed, Scenario::Pay)?;
    if !same_match(&paid, &pay_replay) {
        return Err("Convolute pay-branch full-game replay was not deterministic".to_owned());
    }
    let pristine = mutated_definition(Mutation::None);
    let wrong_payment = mutated_definition(Mutation::WrongPayment);
    let wrong_payment_detected = !definition_identity_valid(&wrong_payment);
    let wrong_color = mutated_definition(Mutation::WrongColor);
    let wrong_color_detected = !definition_identity_valid(&wrong_color);
    let unconditional = mutated_definition(Mutation::Unconditional);
    let unconditional_detected = !definition_identity_valid(&unconditional);
    if !definition_identity_valid(&pristine)
        || !wrong_payment_detected
        || !wrong_color_detected
        || !unconditional_detected
    {
        return Err("a Convolute semantic mutation escaped identity validation".to_owned());
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":41,"printings":1,"definition_id":CARD_ID,"name":"Convolute"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: blue spell identity and colored payment",
                "117: priority and instant-speed response windows",
                "601.2: casting and targeting a spell on the stack",
                "608.2b-608.3: target legality and spell resolution",
                "701.5: countering spells",
                "106 and 118: explicit payment of generic mana during resolution",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Convolute #41, {2}{U} instant: counter target spell unless its controller pays {4}",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" },
        "mutation_sensitivity":{
            "wrong_four_mana_payment_mutation_detected":wrong_payment_detected,
            "wrong_color_and_cost_mutation_detected":wrong_color_detected,
            "unconditional_counter_mutation_detected":unconditional_detected,
            "sealed_mutation_authority":false
        },
        "traces":[decline_trace,pay_trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
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
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|e| e.to_string())
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"},"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPayment,
    WrongColor,
    Unconditional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    Decline,
    Pay,
}

impl Scenario {
    fn target_id(self) -> &'static str {
        match self {
            Self::Decline => TARGET_ID,
            Self::Pay => PAY_TARGET_ID,
        }
    }

    fn deck_name(self) -> &'static str {
        match self {
            Self::Decline => DECK_P1,
            Self::Pay => PAY_DECK_P1,
        }
    }

    fn trace_id(self) -> &'static str {
        match self {
            Self::Decline => "rav-convolute-four-mana-declined",
            Self::Pay => "rav-convolute-four-mana-paid",
        }
    }
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_convolute::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongPayment => {
            definition.effects = vec![Effect::CounterTargetSpellUnlessControllerPays {
                mana_cost: ManaCost::new(3),
            }]
        }
        Mutation::WrongColor => {
            definition.colors = BTreeSet::from([Color::Red]);
            definition.mana_cost = ManaCost::with_colors(2, [Color::Red]);
            definition.effects = vec![Effect::CounterTargetSpellUnlessControllerPays {
                mana_cost: ManaCost::new(4),
            }];
        }
        Mutation::Unconditional => definition.effects = vec![Effect::CounterTargetSpell],
    }
    definition
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    convolute: Option<ObjectId>,
    target: Option<ObjectId>,
    pilot_threat: Option<ObjectId>,
    convolute_payment: Vec<Color>,
    target_payment: Vec<Color>,
    paid_four: bool,
    zones: [Option<Zone>; 2],
    life: [i64; 2],
}

fn find_trace(scenario: Scenario) -> Result<(u64, Captured), String> {
    let env_key = match scenario {
        Scenario::Decline => "CARDBENCH_CONVOLUTE_DECLINE_SEED",
        Scenario::Pay => "CARDBENCH_CONVOLUTE_PAY_SEED",
    };
    if let Ok(value) = std::env::var(env_key) {
        let seed = value
            .parse::<u64>()
            .map_err(|_| format!("{env_key} must be an unsigned integer"))?;
        let captured = run_match(seed, scenario)?;
        validate_trace(&captured, seed, scenario)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=256 {
        match run_match(seed, scenario)
            .and_then(|captured| validate_trace(&captured, seed, scenario).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced the full-game Convolute {:?} trace: {last_error}",
        scenario
    ))
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted == right.accepted
        && left.rejected == right.rejected
        && left.convolute == right.convolute
        && left.target == right.target
        && left.pilot_threat == right.pilot_threat
        && left.convolute_payment == right.convolute_payment
        && left.target_payment == right.target_payment
        && left.paid_four == right.paid_four
        && left.zones == right.zones
        && left.life == right.life
}

fn run_match(seed: u64, scenario: Scenario) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: PILOT_THREAT_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 18,
            },
        ],
        sideboard: vec![],
    };
    let opponent = match scenario {
        Scenario::Decline => DeckList {
            mainboard: vec![
                DeckEntry {
                    card: TARGET_ID.to_owned(),
                    count: 4,
                },
                DeckEntry {
                    card: "RAV-FOREST".to_owned(),
                    count: 28,
                },
                DeckEntry {
                    card: "RAV-PLAINS".to_owned(),
                    count: 28,
                },
            ],
            sideboard: vec![],
        },
        Scenario::Pay => DeckList {
            mainboard: vec![
                DeckEntry {
                    card: PAY_TARGET_ID.to_owned(),
                    count: 4,
                },
                DeckEntry {
                    card: "RAV-FOREST".to_owned(),
                    count: 28,
                },
                DeckEntry {
                    card: "RAV-PLAINS".to_owned(),
                    count: 28,
                },
            ],
            sideboard: vec![],
        },
    };
    for (name, deck) in [(DECK_P0, &pilot), (scenario.deck_name(), &opponent)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{name} deck has {size} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let submitted = candidate_convolute::definition();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Convolute is absent from RAV registry")? = submitted;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ConvolutePolicy::new(PlayerId(0), scenario)),
        Box::new(ConvolutePolicy::new(PlayerId(1), scenario)),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(48) || accepted >= config.max_policy_moves {
            return Err("Convolute match exceeded its bounded turn or move limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("missing seat policy")?;
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
            .map_err(|e| {
                format!(
                    "policy move rejected on turn {} at {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
    }
    let events = game.event_log.clone();
    let find_spell = |seat: PlayerId, id: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat && game.card_definition(*card).is_ok_and(|def| def.id == id) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let convolute = find_spell(PlayerId(0), CARD_ID);
    let target = find_spell(PlayerId(1), scenario.target_id());
    let pilot_threat = find_spell(PlayerId(0), PILOT_THREAT_ID);
    let payment = |player: PlayerId, card: Option<ObjectId>| {
        card.and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: p,
                    card: c,
                    colors,
                } if *p == player && *c == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
    };
    let convolute_payment = payment(PlayerId(0), convolute);
    let target_payment = payment(PlayerId(1), target);
    let paid_four = events.iter().any(|event| matches!(event,
        GameEvent::CounterUnlessPaysManaPaid { player: PlayerId(1), source, target_spell, mana_spent, .. }
            if Some(*source) == convolute && Some(*target_spell) == target && mana_spent.len() == 4
    ));
    let end_of_game = events
        .iter()
        .position(|event| matches!(event, GameEvent::GameEnded { .. }))
        .unwrap_or(events.len());
    let event_zone = |card: Option<ObjectId>| {
        card.and_then(|card| {
            events[..end_of_game]
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
    let zones = [event_zone(convolute), event_zone(target)];
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        convolute,
        target,
        pilot_threat,
        convolute_payment,
        target_payment,
        paid_four,
        zones,
        life: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct ConvolutePolicy {
    player: PlayerId,
    scenario: Scenario,
    target_cast: bool,
}
impl ConvolutePolicy {
    fn new(player: PlayerId, scenario: Scenario) -> Self {
        Self {
            player,
            scenario,
            target_cast: false,
        }
    }
}
impl CodePolicy for ConvolutePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.convolute-counter-and-win.v1"
        } else {
            "rav.watchwolf-target-policy.v1"
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
                    .filter(|c| c.definition == Some(PILOT_THREAT_ID) && c.can_attack)
                    .map(|c| c.id)
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
            && view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(self.scenario.target_id()))
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(2, [Color::Blue]);
                if let Some(mana_selection) = payment(view, &cost) {
                    let target = view
                        .stack_spells
                        .iter()
                        .find(|spell| spell.definition == Some(self.scenario.target_id()))
                        .expect("target");
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![Target::Spell(target.id)],
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
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            let opponent_cast_turn = 12;
            if self.player == PlayerId(1) && !self.target_cast && view.turn >= opponent_cast_turn {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|c| c.definition == Some(self.scenario.target_id()))
                {
                    let cost = if self.scenario == Scenario::Decline {
                        ManaCost::with_colors(0, [Color::Green, Color::White])
                    } else {
                        ManaCost::with_colors(0, [Color::Green])
                    };
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
                    .find(|c| c.definition == Some(PILOT_THREAT_ID))
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
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
        }
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::CounterUnlessPaysMana {
            return None;
        }
        if self.scenario == Scenario::Decline {
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::CounterUnlessPaysMana {
                    pay: false,
                    mana_abilities: vec![],
                    mana_selection: ManaPaymentSelection::default(),
                },
            });
        }
        let mut mana_abilities = Vec::new();
        let mut generic = Vec::new();
        for land in view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            .take(4)
        {
            let color = *land.mana_colors.iter().next()?;
            mana_abilities.push(ResolutionPaymentManaAbility::IntrinsicLand(
                BasicLandManaAbilityActivation {
                    land: land.id,
                    color,
                },
            ));
            generic.push(color);
        }
        (mana_abilities.len() == 4).then_some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::CounterUnlessPaysMana {
                pay: true,
                mana_abilities,
                mana_selection: ManaPaymentSelection {
                    generic,
                    hybrid: vec![],
                },
            },
        })
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|c| (c, view.mana_pool.amount(c)))
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
        let required = cost.colored.iter().filter(|c| *c == color).count() as u8;
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
        .find(|c| {
            c.card_types.contains(&CardType::Land)
                && !c.tapped
                && c.mana_colors.contains(&preferred)
        })
        .or_else(|| {
            view.own_battlefield.iter().find(|c| {
                c.card_types.contains(&CardType::Land) && !c.tapped && !c.mana_colors.is_empty()
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

fn validate_trace(c: &Captured, seed: u64, scenario: Scenario) -> Result<Value, String> {
    let convolute = c.convolute.ok_or("Convolute was not cast")?;
    let target = c.target.ok_or("opposing target spell was not cast")?;
    let pilot = c
        .pilot_threat
        .ok_or("pilot Watchwolf terminal threat was not cast")?;
    let expected_target_zone = if scenario == Scenario::Decline {
        Zone::Graveyard
    } else {
        Zone::Battlefield
    };
    if c.zones != [Some(Zone::Graveyard), Some(expected_target_zone)] {
        return Err(format!(
            "Convolute or its target did not reach the branch-specific final zone for {scenario:?}: {:?}",
            c.zones,
        ));
    }
    if c.convolute_payment.len() != 3 || !c.convolute_payment.contains(&Color::Blue) {
        return Err(format!(
            "Convolute did not pay actual {{2}}{{U}}: {:?}",
            c.convolute_payment
        ));
    }
    let target_payment_valid = if scenario == Scenario::Decline {
        c.target_payment.len() == 2
            && c.target_payment.contains(&Color::Green)
            && c.target_payment.contains(&Color::White)
    } else {
        c.target_payment.len() == 1 && c.target_payment.contains(&Color::Green)
    };
    if !target_payment_valid {
        return Err(format!(
            "opposing target spell did not pay its printed colored cost: {:?}",
            c.target_payment
        ));
    }
    let target_paid = event_index(&c.events, |e| match (scenario, e) {
        (
            Scenario::Decline,
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            },
        ) => *card == target && colors.contains(&Color::Green) && colors.contains(&Color::White),
        (
            Scenario::Pay,
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            },
        ) => *card == target && colors.len() == 1 && colors.contains(&Color::Green),
        _ => false,
    })
    .ok_or("target spell mana payment receipt missing")?;
    let target_cast = event_index_after(
        &c.events,
        target_paid,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(1),card}if *card==target),
    )
    .ok_or("target Watchwolf cast receipt missing")?;
    let con_paid=event_index_after(&c.events,target_cast,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==convolute&&colors.len()==3&&colors.contains(&Color::Blue))).ok_or("Convolute's {2}{U} payment receipt missing")?;
    let con_cast = event_index_after(
        &c.events,
        con_paid,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==convolute),
    )
    .ok_or("Convolute cast receipt missing")?;
    let opened = event_index_after(&c.events, con_cast, |e| {
        matches!(
            e,
            GameEvent::DecisionOpened {
                player: PlayerId(1),
                kind: DecisionKind::CounterUnlessPaysMana,
                ..
            }
        )
    })
    .ok_or("target controller's explicit {4} payment decision missing")?;
    let (branch_event, target_moved) = if scenario == Scenario::Decline {
        if c.paid_four {
            return Err("target controller paid four in the decline trace".to_owned());
        }
        let countered = event_index_after(&c.events, opened, |e| {
            matches!(e, GameEvent::SpellCountered { card, source } if *card == target && *source == convolute)
        })
        .ok_or("Convolute did not counter its exact target after payment was declined")?;
        let moved = event_index_after(&c.events, countered, |e| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == target)
        })
        .ok_or("countered target did not move to its owner's graveyard")?;
        (countered, moved)
    } else {
        if !c.paid_four {
            return Err("target controller did not pay four in the pay trace".to_owned());
        }
        if c.events.iter().any(|e| matches!(e, GameEvent::SpellCountered { card, source } if *card == target && *source == convolute)) {
            return Err("Convolute countered its target despite payment".to_owned());
        }
        let paid = event_index_after(&c.events, opened, |e| {
            matches!(e, GameEvent::CounterUnlessPaysManaPaid { player: PlayerId(1), source, target_spell, mana_spent, .. } if *source == convolute && *target_spell == target && mana_spent.len() == 4)
        })
        .ok_or("explicit four-colorless payment receipt missing")?;
        let target_resolved = event_index_after(
            &c.events,
            paid,
            |e| matches!(e, GameEvent::SpellResolved { card } if *card == target),
        )
        .ok_or("paid-for target spell did not resolve")?;
        let moved = event_index_after(&c.events, target_resolved, |e| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == target)
        })
        .ok_or("paid-for target spell did not enter the battlefield")?;
        (paid, moved)
    };
    let resolved = event_index_after(
        &c.events,
        branch_event,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == convolute),
    )
    .ok_or("Convolute did not finish resolving")?;
    if !(target_paid < target_cast
        && target_cast < con_paid
        && con_paid < con_cast
        && con_cast < opened
        && opened < branch_event
        && branch_event < target_moved
        && branch_event < resolved)
    {
        return Err("target cast, instant response, payment decision, resolution, and cleanup order differs".to_owned());
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life[0] <= 0
        || c.life[1] > 0
        || c.rejected != 0
    {
        return Err(format!(
            "Convolute PvP game did not reach a clean seat-zero win: {:?}",
            c.life
        ));
    }
    let attack=event_index(&c.events,|e|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&pilot))).ok_or("pilot Watchwolf did not attack")?;
    let damage=event_index_after(&c.events,attack,|e|matches!(e,GameEvent::DamageDealtToPlayer{source,player:PlayerId(1),amount}if *source==pilot&&*amount>0)).ok_or("pilot Watchwolf did not deal combat damage")?;
    let ended = event_index_after(&c.events, damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal game result event missing")?;
    let projected: Vec<Value> = project_events(&c.events)
        .iter()
        .map(|e| json!({"sequence":e.sequence,"turn":e.turn,"kind":e.kind,"canonical":e.canonical}))
        .collect();
    let ordered_event_sequence = if scenario == Scenario::Pay {
        vec![
            target_paid,
            target_cast,
            con_paid,
            con_cast,
            opened,
            branch_event,
            resolved,
            target_moved,
            attack,
            damage,
            ended,
        ]
    } else {
        vec![
            target_paid,
            target_cast,
            con_paid,
            con_cast,
            opened,
            branch_event,
            target_moved,
            resolved,
            attack,
            damage,
            ended,
        ]
    };
    Ok(
        json!({"id":scenario.trace_id(),"branch":if scenario==Scenario::Decline{"decline"}else{"pay"},"passed":true,"seed":seed,"decks":[DECK_P0,scenario.deck_name()],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"convolute_object":convolute.0,"target_spell_object":target.0,"pilot_watchwolf_object":pilot.0,"convolute_cast_payment":{"colors":c.convolute_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"blue_paid":c.convolute_payment.contains(&Color::Blue)},"target_spell_cast_payment":{"colors":c.target_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"green_paid":c.target_payment.contains(&Color::Green),"white_paid":c.target_payment.contains(&Color::White)},"target_controller_declined_four_mana":scenario==Scenario::Decline,"target_controller_paid_four_mana":c.paid_four,"target_spell_zone":format!("{:?}",c.zones[1]),"convolute_zone":format!("{:?}",c.zones[0]),"life_at_match_end":c.life,"ordered_event_sequence":ordered_event_sequence,"events":projected}),
    )
}

fn event_index(events: &[GameEvent], mut pred: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|e| pred(e))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut pred: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(i, e)| pred(e).then_some(i))
}

fn definition_identity_valid(card: &CardDefinition) -> bool {
    card.id == CARD_ID
        && card.name == "Convolute"
        && card.set_code == "RAV"
        && card.mana_cost == ManaCost::with_colors(2, [Color::Blue])
        && card.colors == BTreeSet::from([Color::Blue])
        && card.mana_colors.is_empty()
        && card.card_types == BTreeSet::from([CardType::Instant])
        && !card.is_basic_land
        && card.supported_rules
            == [
                "full-rules-fidelity",
                "counter-target-spell-unless-controller-pays-4",
                "policy-submitted-resolution-payment",
            ]
        && card.power.is_none()
        && card.toughness.is_none()
        && card.keywords.is_empty()
        && card.effects
            == [Effect::CounterTargetSpellUnlessControllerPays {
                mana_cost: ManaCost::new(4),
            }]
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Convolute source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Convolute source is not valid Rust syntax".to_owned())?;
    if file.items.len()!=3||!file.items.iter().any(|item|matches!(item,Item::Fn(function)if function.sig.ident=="definition"&&matches!(&function.vis,Visibility::Restricted(v)if v.path.is_ident("crate"))&&function.sig.inputs.is_empty()&&function.block.stmts.len()==1&&matches!(&function.block.stmts[0],Stmt::Expr(Expr::Struct(value),None)if value.path.segments.last().is_some_and(|segment|segment.ident=="CardDefinition")&&value.rest.is_none()))) {
        return Err("Convolute source must contain imports and one literal definition()".to_owned());
    }
    let candidate = candidate_convolute::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Convolute is absent from RAV registry")?;
    if candidate != registered || !definition_identity_valid(&candidate) {
        return Err("candidate Convolute definition differs from its RAV identity".to_owned());
    }
    Ok(())
}
