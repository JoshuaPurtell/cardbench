//! Full-game RAV Greater Forgeling authority for its {1}{R} +3/-3 pump.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardType, Color, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, Layer, ManaCost, ObjectId, PlayerId, PolicyAction, Step, Zone,
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
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GREATER-FORGELING";
const ABILITY_ID: &str = "pump-plus-three-minus-three";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/greater_forgeling.rs";
const DECK_P0: &str = "rav_greater_forgeling_self_pump";
const DECK_P1: &str = "rav_basic_plains_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/greater_forgeling.rs"]
mod candidate_greater_forgeling;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Greater Forgeling PvP result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Greater Forgeling PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Greater Forgeling full-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    if [
        "wrong_pump_power_mutation_detected_in_game",
        "wrong_pump_toughness_mutation_detected_in_game",
        "wrong_ability_cost_mutation_detected_in_game",
        "wrong_base_power_mutation_detected_in_game",
        "wrong_cast_cost_mutation_detected_in_game",
    ]
    .iter()
    .any(|key| mutations[*key] != true)
    {
        return Err("a Greater Forgeling semantic mutation escaped its PvP trace".to_owned());
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
        "card":{"collector_number":129,"definition_id":CARD_ID,"name":"Greater Forgeling"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: red spell color and colored payment",
                "106.1 and 106.4: mana production and activated-ability payment",
                "117: priority and activated-ability response windows",
                "602.2: activating an activated ability",
                "601.2: casting and paying the creature spell",
                "605: mana abilities",
                "608.2-608.3: ability resolution and continuous effect",
                "613.4: power/toughness modification layer",
                "514: cleanup and expiration of until-end-of-turn effects",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Greater Forgeling #129, {3}{R}{R} 3/4 red creature, and candidate-owned {1}{R}: +3/-3 until end of turn",
            "authority_cutoff":"2026-09-23",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.forgeling == right.forgeling
        && left.cast_payment == right.cast_payment
        && left.first_activation_turn == right.first_activation_turn
        && left.first_live_pt == right.first_live_pt
        && left.first_cleanup_pt == right.first_cleanup_pt
        && left.life_totals == right.life_totals
        && left.attack_turns == right.attack_turns
        && left.red_ability_payments == right.red_ability_payments
        && left.resolution_receipts == right.resolution_receipts
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
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoPower,
    WrongToughness,
    WrongAbilityCost,
    WrongBasePower,
    WrongCastCost,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    forgeling: Option<ObjectId>,
    cast_payment: Vec<Color>,
    first_activation_turn: Option<u32>,
    first_live_pt: Option<(i32, i32)>,
    first_cleanup_pt: Option<(i32, i32)>,
    life_totals: [i64; 2],
    attack_turns: Vec<u32>,
    red_ability_payments: usize,
    resolution_receipts: usize,
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..512 {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("was not drawn") || error.contains("was not cast") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a full-game Greater Forgeling pump trace".to_owned())
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (name, mutation) in [
        (
            "wrong_pump_power_mutation_detected_in_game",
            Mutation::NoPower,
        ),
        (
            "wrong_pump_toughness_mutation_detected_in_game",
            Mutation::WrongToughness,
        ),
        (
            "wrong_ability_cost_mutation_detected_in_game",
            Mutation::WrongAbilityCost,
        ),
        (
            "wrong_base_power_mutation_detected_in_game",
            Mutation::WrongBasePower,
        ),
        (
            "wrong_cast_cost_mutation_detected_in_game",
            Mutation::WrongCastCost,
        ),
    ] {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(name.to_owned(), Value::Bool(detected));
    }
    outcomes.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    Ok(Value::Object(outcomes))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
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
    let submitted = candidate_greater_forgeling::definition();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Greater Forgeling definition is absent from RAV registry")? = submitted;
    let mut ability = candidate_greater_forgeling::pump_ability();
    if mutation == Mutation::NoPower {
        ability.effects = vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 0,
            toughness: -3,
        }];
    } else if mutation == Mutation::WrongToughness {
        ability.effects = vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 3,
            toughness: -2,
        }];
    } else if mutation == Mutation::WrongAbilityCost {
        ability.mana_cost = ManaCost::with_colors(0, [Color::Red]);
    }
    if mutation == Mutation::WrongBasePower {
        let submitted = definitions
            .iter_mut()
            .find(|definition| definition.id == CARD_ID)
            .ok_or("Greater Forgeling definition is absent from RAV registry")?;
        *submitted = candidate_greater_forgeling::definition();
        submitted.power = Some(4);
    } else if mutation == Mutation::WrongCastCost {
        let submitted = definitions
            .iter_mut()
            .find(|definition| definition.id == CARD_ID)
            .ok_or("Greater Forgeling definition is absent from RAV registry")?;
        *submitted = candidate_greater_forgeling::definition();
        submitted.mana_cost = ManaCost::with_colors(4, [Color::Red]);
    }
    let mut bindings = rav_activated_ability_bindings();
    bindings.retain(|binding| binding.card_definition != CARD_ID);
    bindings.push(ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability,
    });
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        bindings,
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
        Box::new(ForgelingPolicy::new(PlayerId(0))),
        Box::new(ForgelingPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    let mut attack_turns = Vec::new();
    let mut first_live_pt = None;
    let mut first_cleanup_pt = None;
    let mut first_activation_turn = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(60) || accepted >= config.max_policy_moves {
            return Err(
                "Greater Forgeling match exceeded its bounded turn or move limit".to_owned(),
            );
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
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy move rejected for seat {} on turn {} at {:?} (active {}, decision {}, priority {}, attackers {}, blockers {}): {e}",
                    player.0, view.turn, view.step, view.active_player.0, view.decision_player.0,
                    view.priority.0, view.attackers_declared, view.blockers_declared
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        let new_events = &game.event_log[cursor..];
        if new_events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|id| game.card_definition(*id).is_ok_and(|card| card.id == CARD_ID)))) {
            attack_turns.push(game.turn);
        }
        if first_live_pt.is_none()
            && let Some(source) = new_events.iter().find_map(|event| match event {
                GameEvent::AbilityResolved {
                    source,
                    ability: ABILITY_ID,
                    ..
                } => Some(*source),
                _ => None,
            })
        {
            first_activation_turn = Some(game.turn);
            first_live_pt = game
                .characteristics(source)
                .ok()
                .and_then(|c| Some((c.power?, c.toughness?)));
        }
        if first_cleanup_pt.is_none()
            && let Some(source) = new_events.iter().find_map(|event| match event {
                GameEvent::ContinuousEffectExpired {
                    source,
                    target,
                    layer: Layer::PowerToughness,
                } if source == target => Some(*source),
                _ => None,
            })
        {
            first_cleanup_pt = game
                .characteristics(source)
                .ok()
                .and_then(|c| Some((c.power?, c.toughness?)));
        }
    }
    let events = game.event_log.clone();
    let forgeling = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let cast_payment = forgeling
        .and_then(|source| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == source => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let red_ability_payments = forgeling.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: ABILITY_ID, mana_cost } if *paid_source == source && mana_cost == &ManaCost::with_colors(1, [Color::Red])
    )).count());
    let resolution_receipts = forgeling.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityResolved { source: paid_source, ability: ABILITY_ID, .. } if *paid_source == source
    )).count());
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        forgeling,
        cast_payment,
        first_activation_turn,
        first_live_pt,
        first_cleanup_pt,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
        attack_turns,
        red_ability_payments,
        resolution_receipts,
    })
}

struct ForgelingPolicy {
    player: PlayerId,
    activated_turns: BTreeSet<u32>,
}
impl ForgelingPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activated_turns: BTreeSet::new(),
        }
    }
}
impl CodePolicy for ForgelingPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.greater-forgeling-self-pump.v1"
        } else {
            "rav.watchwolf-pressure.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                .map(|card| card.id)
                .collect();
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
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        let lands_on_battlefield = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land))
            .count();
        if view.lands_played == 0 && lands_on_battlefield < 5 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(1) {
            return PolicyAction::PassPriority;
        }
        let forgeling = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID));
        if forgeling.is_none() {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Red, Color::Red]);
                if let Some(selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        if !self.activated_turns.contains(&view.turn) {
            if let Some(source) = forgeling {
                let cost = ManaCost::with_colors(1, [Color::Red]);
                if payment(view, &cost).is_some() {
                    self.activated_turns.insert(view.turn);
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment(
    view: &GameView,
    cost: &ManaCost,
) -> Option<cardbench_magic_engine::ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(available_color, _)| available_color == color)?;
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
    Some(cardbench_magic_engine::ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}
fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let mut need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0);
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Red);
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
    let forgeling = c.forgeling.ok_or("Greater Forgeling was not cast")?;
    if c.cast_payment.len() != 5
        || c.cast_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            < 2
    {
        return Err(format!(
            "Greater Forgeling did not pay its {{3}}{{R}}{{R}} casting cost: {:?}",
            c.cast_payment
        ));
    }
    if c.first_live_pt != Some((6, 1)) {
        return Err("Greater Forgeling's pump did not create a live 6/1".to_owned());
    }
    if c.first_cleanup_pt != Some((3, 4)) {
        return Err(
            "Greater Forgeling did not return to 3/4 after the until-end-of-turn pump expired"
                .to_owned(),
        );
    }
    if c.red_ability_payments < 1 || c.resolution_receipts < 1 {
        return Err(
            "Greater Forgeling's {1}{R} ability payment and stack resolution were not recorded"
                .to_owned(),
        );
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Greater Forgeling PvP game did not reach a clean seat-zero terminal win: {:?}",
            c.life_totals
        ));
    }
    if c.attack_turns.len() < 3 {
        return Err("Greater Forgeling did not complete repeated combat after its pump".to_owned());
    }
    let powered_attacks = c.events.iter().filter(|event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 6 } if *source == forgeling
    )).count();
    if powered_attacks != c.attack_turns.len() || !has_same_turn_pump_combat(&c.events, forgeling) {
        return Err(
            "Greater Forgeling's resolved +3/-3 pump was not proven by same-turn 6-damage combat"
                .to_owned(),
        );
    }
    let paid = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == forgeling && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Red).count() >= 2)).ok_or("Greater Forgeling exact colored cast-payment receipt is missing")?;
    let cast = event_index_after(&c.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == forgeling)).ok_or("Greater Forgeling cast receipt is missing")?;
    let spell_resolved = event_index_after(
        &c.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == forgeling),
    )
    .ok_or("Greater Forgeling did not resolve")?;
    let entered = event_index_after(&c.events, spell_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == forgeling)).ok_or("Greater Forgeling did not enter the battlefield")?;
    let mana_paid = event_index_after(&c.events, entered, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == forgeling && mana_cost == &ManaCost::with_colors(1, [Color::Red]))).ok_or("Greater Forgeling exact {1}{R} ability-payment receipt is missing")?;
    let activation = event_index_after(&c.events, mana_paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == forgeling)).ok_or("Greater Forgeling ability did not enter the stack")?;
    let resolved = event_index_after(&c.events, activation, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == forgeling)).ok_or("Greater Forgeling pump did not resolve")?;
    let attack = event_index_after(&c.events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&forgeling))).ok_or("Greater Forgeling did not attack after its pump resolved")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 6 } if *source == forgeling)).ok_or("live 6/1 Greater Forgeling did not deal 6 combat damage")?;
    let ended = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero game event is missing")?;
    if !(paid < cast
        && cast < spell_resolved
        && spell_resolved < entered
        && entered < mana_paid
        && mana_paid < activation
        && activation < resolved
        && resolved < attack
        && attack < damage
        && damage < ended)
    {
        return Err(
            "Greater Forgeling cast, payment, activation, stack, and combat event order is invalid"
                .to_owned(),
        );
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(json!({
        "trace_id":"rav-greater-forgeling-pump-combat-full-game",
        "passed":true,
        "shuffle_seed":seed,
        "decks":[DECK_P0,DECK_P1],
        "terminal":c.terminal,
        "winner_seat":c.winner,
        "turns":c.turns,
        "accepted_moves":c.accepted_moves,
        "rejected_moves":c.rejected_moves,
        "forgeling":{"object":forgeling.0,"mana_cost":"{3}{R}{R}","cast_mana_paid":c.cast_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"zone":"Battlefield","base_power":3,"base_toughness":4,"cast_turn":projected[cast].turn},
        "activation":{"ability":ABILITY_ID,"mana_cost":"{1}{R}","paid_activations":c.red_ability_payments,"resolutions":c.resolution_receipts,"first_live_power":c.first_live_pt.map(|pt|pt.0),"first_live_toughness":c.first_live_pt.map(|pt|pt.1),"power_after_cleanup":c.first_cleanup_pt.map(|pt|pt.0),"toughness_after_cleanup":c.first_cleanup_pt.map(|pt|pt.1)},
        "combat":{"attack_turns":c.attack_turns,"powered_attack_count":powered_attacks,"damage_per_attack":6,"same_turn_pump_attack":true,"life_at_terminal":c.life_totals},
        "ordered_event_sequence":[paid,cast,spell_resolved,entered,mana_paid,activation,resolved,attack,damage,ended],
        "events":events
    }))
}

fn event_index(events: &[GameEvent], mut pred: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|event| pred(event))
}

fn has_same_turn_pump_combat(events: &[GameEvent], forgeling: ObjectId) -> bool {
    let projected = project_events(events);
    events.iter().enumerate().any(|(resolved_index, event)| {
        if !matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == forgeling) {
            return false;
        }
        let turn = projected[resolved_index].turn;
        let Some(attack_index) = events.iter().enumerate().skip(resolved_index + 1).find_map(|(index, event)| {
            (projected[index].turn == turn && matches!(event,
                GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&forgeling)
            )).then_some(index)
        }) else { return false; };
        events.iter().enumerate().skip(attack_index + 1).any(|(index, event)| {
            projected[index].turn == turn && matches!(event,
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 6 } if *source == forgeling
            )
        })
    })
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
        .find_map(|(index, event)| pred(event).then_some(index))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Greater Forgeling source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Greater Forgeling source is not valid declarative Rust")?;
    if !file
        .attrs
        .iter()
        .all(|attr| attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
    {
        return Err("Greater Forgeling module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) => {
                if !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty() {
                    return Err("Greater Forgeling imports must be private".to_owned());
                }
                imports.extend(
                    flattened_use_paths(item).ok_or("Greater Forgeling import shape differs")?,
                );
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !["definition", "pump_ability"].contains(&name.as_str())
                    || !matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attr| {
                        (attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
                            || attr.path().is_ident("must_use")
                    })
                {
                    return Err(
                        "Greater Forgeling must expose only safe no-argument candidate functions"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Greater Forgeling functions must return direct literals".to_owned()
                    );
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Greater Forgeling source contains an unsupported executable expression"
                            .to_owned(),
                    );
                }
                functions.insert(name);
            }
            _ => {
                return Err(
                    "Greater Forgeling module permits only imports and the two candidate functions"
                        .to_owned(),
                );
            }
        }
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if functions != BTreeSet::from(["definition".to_owned(), "pump_ability".to_owned()])
        || imports != expected
    {
        return Err("Greater Forgeling source imports or candidate functions differ".to_owned());
    }
    let card = candidate_greater_forgeling::definition();
    if card.id != CARD_ID
        || card.name != "Greater Forgeling"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(3, [Color::Red, Color::Red])
        || card.colors != BTreeSet::from([Color::Red])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.power != Some(3)
        || card.toughness != Some(4)
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Greater Forgeling identity, {1}{R} cost, or base characteristics differ".to_owned(),
        );
    }
    let ability = candidate_greater_forgeling::pump_ability();
    if ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects
            != [Effect::ModifySourcePtUntilEndOfTurn {
                power: 3,
                toughness: -3,
            }]
    {
        return Err("Greater Forgeling's {1}{R} +3/-3 activation differs".to_owned());
    }
    Ok(())
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_greater_forgeling::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Greater Forgeling definition is missing from the RAV registry")?;
    if registered != candidate {
        return Err(
            "Greater Forgeling registry definition does not delegate to its candidate module"
                .to_owned(),
        );
    }
    let ability = candidate_greater_forgeling::pump_ability();
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Greater Forgeling pump ability is missing from the RAV registry")?;
    if registered_ability.ability != ability {
        return Err(
            "Greater Forgeling registry ability does not delegate to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
                && value.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a,b] if (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "ManaCost" && (b == "new" || b == "with_colors")))
                        || matches!(names.as_slice(), [name] if name == "Some")
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
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
