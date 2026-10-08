//! Full-game RAV Carrion Howler authority for its life-paid temporary pump.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CardType, Color, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, GeneralizedAbilityActivation, GeneralizedActivatedAbilityCost, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CARRION-HOWLER";
const ABILITY_ID: &str = "pay-life-pump-plus-two-minus-one";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/carrion_howler.rs";
const DECK_P0: &str = "rav_carrion_howler_life_pump";
const DECK_P1: &str = "rav_basic_land_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/carrion_howler.rs"]
mod candidate_carrion_howler;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Carrion Howler PvP result: {error}");
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
            eprintln!("Carrion Howler PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.howler != replay.howler
        || captured.cast_payment != replay.cast_payment
        || captured.life_totals != replay.life_totals
        || captured.first_live_pt != replay.first_live_pt
        || captured.after_cleanup_pt != replay.after_cleanup_pt
        || captured.attack_turns != replay.attack_turns
    {
        return Err("Carrion Howler full-game replay was not deterministic".to_owned());
    }
    let effect_mutation_detected = mutation_reaches_trace_and_fails(seed, Mutation::WrongPump)?;
    let life_cost_mutation_detected =
        mutation_reaches_trace_and_fails(seed, Mutation::WrongLifeCost)?;
    if !effect_mutation_detected || !life_cost_mutation_detected {
        return Err(
            "a Carrion Howler targeted semantic mutation escaped its trace gate".to_owned(),
        );
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
        "card": {"collector_number": 79, "definition_id": CARD_ID, "name": "Carrion Howler"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: black color identity and colored payment",
                "106.1 and 106.4: mana production and spell payment",
                "117.1b and 117.3c: priority, activation, and responses",
                "302.1-302.6: creature permanents and summoning sickness",
                "508 and 510: attacking and combat damage",
                "601.2 and 608.3a: casting, costs, and permanent resolution",
                "602.1-602.2: activated-ability costs and stack resolution",
                "611.2a and 613: until-end-of-turn characteristic effects",
                "119.4: paying life as a cost"
            ],
            "source_contract": "RAV Carrion Howler identity, {3}{B} cost, 2/2 body, and candidate-owned pay-one-life +2/-1 activated ability",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity": {
            "wrong_temporary_pump_effect_mutation_detected": effect_mutation_detected,
            "wrong_life_payment_mutation_detected": life_cost_mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
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

fn output_root() -> Option<std::path::PathBuf> {
    let arguments: Vec<_> = std::env::args_os().collect();
    arguments
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPump,
    WrongLifeCost,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    howler: Option<ObjectId>,
    cast_payment: Vec<Color>,
    life_totals: [i64; 2],
    first_live_pt: Option<(i32, i32)>,
    after_cleanup_pt: Option<(i32, i32)>,
    first_activation_life: Option<i64>,
    first_attack_turn: Option<u32>,
    attack_turns: Vec<u32>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..256 {
        match run_match(seed, Mutation::None) {
            Ok(captured) => match validate_trace(&captured, seed) {
                Ok(_) => return Ok((seed, captured)),
                Err(error)
                    if error.contains("was not drawn")
                        || error.contains("was not cast")
                        || error.contains("did not reach its live") => {}
                Err(error) => return Err(error),
            },
            Err(error) if error.contains("not drawn") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a complete Carrion Howler life-pump game".to_owned())
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let captured = run_match(seed, mutation)?;
    let error = validate_trace(&captured, seed).err().unwrap_or_default();
    let expected = match mutation {
        Mutation::WrongPump => "Carrion Howler's live pump did not make it a 4/1 until cleanup",
        Mutation::WrongLifeCost => {
            "Carrion Howler did not pay exactly 1 life for its stack activation"
        }
        Mutation::None => return Ok(false),
    };
    Ok(error.contains(expected))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{label} deck has {size} cards; expected 60"));
        }
    }
    let definition = candidate_carrion_howler::definition();
    let ability = candidate_carrion_howler::life_pump_ability();
    let cost = candidate_carrion_howler::life_pump_cost();
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Carrion Howler is missing from the RAV definition registry")? = definition.clone();
    let mut abilities = rav_activated_ability_bindings();
    let registered = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Carrion Howler activated ability is missing from the RAV registry")?;
    registered.ability = if mutation == Mutation::WrongPump {
        let mut altered = ability.clone();
        altered.effects = vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 1,
            toughness: -1,
        }];
        altered
    } else {
        ability.clone()
    };
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    let mut costs = rav_generalized_activated_ability_cost_bindings();
    costs.retain(|binding| {
        !(binding.card_definition == CARD_ID && binding.ability_id == ABILITY_ID)
    });
    let mut candidate_cost = cost;
    if mutation == Mutation::WrongLifeCost {
        candidate_cost.cost.life_payment = 2;
    }
    costs.push(candidate_cost);
    game.register_generalized_activated_ability_cost_bindings(costs)
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
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(HowlerPolicy::new(PlayerId(0), definition.mana_cost)),
        Box::new(PassPolicy),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut first_live_pt = None;
    let mut after_cleanup_pt = None;
    let mut first_activation_life = None;
    let mut first_attack_turn = None;
    let mut attack_turns = Vec::new();
    let mut first_activation_turn = None;
    let max_turns = config.max_turns.min(40);
    while !game.is_game_over() {
        if game.turn > max_turns || accepted >= config.max_policy_moves {
            return Err("Carrion Howler match reached a development bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
        let event_cursor = game.event_log.len();
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
        let events = &game.event_log;
        if first_live_pt.is_none() {
            if let Some(source) = events.iter().find_map(|event| match event {
                GameEvent::AbilityResolved {
                    source,
                    ability: ABILITY_ID,
                    ..
                } => Some(*source),
                _ => None,
            }) {
                first_live_pt = game
                    .characteristics(source)
                    .ok()
                    .and_then(|chars| Some((chars.power?, chars.toughness?)));
                first_activation_turn = Some(game.turn);
                first_activation_life = game.player(PlayerId(0)).ok().map(|p| p.life);
            }
        }
        let new_events = &game.event_log[event_cursor..];
        if new_events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|id| game.card_definition(*id).is_ok_and(|card| card.id == CARD_ID)))) {
            first_attack_turn.get_or_insert(game.turn);
            attack_turns.push(game.turn);
        }
        if after_cleanup_pt.is_none() && first_activation_turn.is_some_and(|turn| game.turn > turn)
        {
            if let Some(source) = events.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            }) {
                let pt = game
                    .characteristics(source)
                    .ok()
                    .and_then(|chars| Some((chars.power?, chars.toughness?)));
                if pt == Some((2, 2)) {
                    after_cleanup_pt = pt;
                }
            }
        }
    }
    let events = game.event_log.clone();
    let howler = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let cast_payment = howler
        .and_then(|howler| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == howler => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |player| format!("Winner(PlayerId({}))", player.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        howler,
        cast_payment,
        life_totals: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
        first_live_pt,
        after_cleanup_pt,
        first_activation_life,
        first_attack_turn,
        attack_turns,
    })
}

struct HowlerPolicy {
    player: PlayerId,
    cost: ManaCost,
    pumped_turn: Option<u32>,
}
impl HowlerPolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            cost,
            pumped_turn: None,
        }
    }
}
struct PassPolicy;

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.carrion_howler-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.active_player == view.player && !view.attackers_declared =>
            {
                PolicyAction::DeclareAttackers { attackers: vec![] }
            }
            Step::DeclareBlockers
                if view.active_player != view.player && !view.blockers_declared =>
            {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            }
            _ => PolicyAction::PassPriority,
        }
    }
}

impl CodePolicy for HowlerPolicy {
    fn id(&self) -> &'static str {
        "rav.carrion_howler-cast-pump-and-attack.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step != Step::PrecombatMain
            || view.active_player != self.player
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
        if self.pumped_turn != Some(view.turn) {
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && !card.summoning_sick)
            {
                self.pumped_turn = Some(view.turn);
                return PolicyAction::ActivateAbilityWithGeneralizedCosts {
                    activation: GeneralizedAbilityActivation {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                        cost_payment: AbilityCostPayment::default(),
                        mana_payment_selection: None,
                    },
                };
            }
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
        {
            if let Some(payment) = payment(view, &self.cost) {
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: payment,
                };
            }
        }
        if let Some(land) = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&Color::Black)
        }) {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Black,
            };
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
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

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(
            "Carrion Howler game did not reach a clean terminal player-zero win".to_owned(),
        );
    }
    let howler = captured.howler.ok_or("Carrion Howler was not cast")?;
    if captured.cast_payment.len() != 4 || !captured.cast_payment.contains(&Color::Black) {
        return Err(format!(
            "Carrion Howler did not pay its actual {{3}}{{B}} cast cost: {:?}",
            captured.cast_payment
        ));
    }
    if captured.first_live_pt != Some((4, 1)) {
        return Err("Carrion Howler's live pump did not make it a 4/1 until cleanup".to_owned());
    }
    if captured.after_cleanup_pt != Some((2, 2)) {
        return Err("Carrion Howler's +2/-1 effect did not expire at cleanup".to_owned());
    }
    if captured.first_activation_life != Some(19) {
        return Err(
            "Carrion Howler did not pay exactly 1 life for its stack activation".to_owned(),
        );
    }
    let cast_payment = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == howler && colors.len() == 4 && colors.contains(&Color::Black))).ok_or("Carrion Howler actual four-mana colored payment receipt missing")?;
    let cast = event_index_after(&captured.events, cast_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == howler)).ok_or("Carrion Howler cast receipt missing")?;
    let spell_resolved = event_index_after(
        &captured.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == howler),
    )
    .ok_or("Carrion Howler creature spell did not resolve")?;
    let entered = event_index_after(&captured.events, spell_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == howler)).ok_or("Carrion Howler did not enter the battlefield")?;
    let life_event = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityLifePaid { player: PlayerId(0), source, ability: ABILITY_ID, amount: 1 } if *source == howler)).ok_or("Carrion Howler life-payment event missing")?;
    if life_event <= entered {
        return Err("Carrion Howler activated before its creature spell resolved".to_owned());
    }
    let activation = event_index_after(&captured.events, life_event, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == howler)).ok_or("Carrion Howler activation did not reach the stack")?;
    let resolved = event_index_after(&captured.events, activation, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == howler)).ok_or("Carrion Howler pump did not resolve")?;
    let attacked = event_index_after(&captured.events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&howler))).ok_or("Carrion Howler did not attack after its pump resolved")?;
    let damage = event_index_after(&captured.events, attacked, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4, .. } if *source == howler)).ok_or("live 4/1 Carrion Howler did not deal four combat damage")?;
    let ended = event_index_after(&captured.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
                ..
            }
        )
    })
    .ok_or("Carrion Howler did not lead to the terminal PvP win")?;
    let first_attack_turn = captured
        .first_attack_turn
        .ok_or("Carrion Howler never attacked")?;
    let life_payment_count = captured.events.iter().filter(|event| matches!(event, GameEvent::AbilityLifePaid { player: PlayerId(0), source, ability: ABILITY_ID, amount: 1 } if *source == howler)).count();
    let activation_count = captured.events.iter().filter(|event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == howler)).count();
    let resolution_count = captured.events.iter().filter(|event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == howler)).count();
    let four_damage_count = captured.events.iter().filter(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == howler)).count();
    if life_payment_count != 5
        || activation_count != 5
        || resolution_count != 5
        || four_damage_count != 5
        || captured.attack_turns.len() != 5
    {
        return Err(format!(
            "Carrion Howler must show five separate paid, resolved 4-damage attacks; life={life_payment_count}, activations={activation_count}, resolutions={resolution_count}, damage={four_damage_count}, attacks={}",
            captured.attack_turns.len()
        ));
    }
    let projected = project_events(&captured.events);
    let event_values: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical})).collect();
    Ok(json!({
        "id":"rav-carrion-howler-life-paid-pump",
        "passed":true,
        "seed":seed,
        "decks":[DECK_P0,DECK_P1],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":captured.rejected_moves,
        "howler_object":howler.0,
        "cast_payment":{"total":captured.cast_payment.len(),"colors":captured.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"black_symbol_paid":captured.cast_payment.contains(&Color::Black)},
        "life_at_activation":captured.first_activation_life,
        "life_at_match_end":captured.life_totals,
        "live_pump_power_toughness":{"power":captured.first_live_pt.map(|pt|pt.0),"toughness":captured.first_live_pt.map(|pt|pt.1)},
        "post_cleanup_power_toughness":{"power":captured.after_cleanup_pt.map(|pt|pt.0),"toughness":captured.after_cleanup_pt.map(|pt|pt.1)},
        "attack_turns":captured.attack_turns,
        "first_attack_turn":first_attack_turn,
        "ordered_event_sequence":[cast_payment,cast,spell_resolved,entered,life_event,activation,resolved,attacked,damage,ended],
        "events":event_values
    }))
}

fn event_index(events: &[GameEvent], mut matches: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|event| matches(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut matches: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| matches(event).then_some(index))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Carrion Howler candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Carrion Howler source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
    }) {
        return Err("Carrion Howler module has a non-documentation attribute".to_owned());
    }
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for item in &file.items {
        if let Item::Use(item) = item {
            if !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty() {
                return Err("Carrion Howler imports must be private and unadorned".to_owned());
            }
            let paths = flattened_use_paths(item).ok_or("Carrion Howler import is unsupported")?;
            imports.extend(paths);
            continue;
        }
        let Item::Fn(function) = item else {
            return Err("Carrion Howler source permits only three literal functions".to_owned());
        };
        if !function.attrs.iter().all(|attribute| {
            (attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_)))
                || attribute.path().is_ident("must_use")
        }) {
            return Err(
                "Carrion Howler functions permit only documentation and must_use attributes"
                    .to_owned(),
            );
        }
        let crate_visible = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
        if !crate_visible
            || function.sig.asyncness.is_some()
            || function.sig.unsafety.is_some()
            || !function.sig.generics.params.is_empty()
            || !function.sig.inputs.is_empty()
            || function.sig.variadic.is_some()
            || function.block.stmts.len() != 1
        {
            return Err(format!(
                "Carrion Howler function {} must be a simple crate-visible no-argument literal",
                function.sig.ident
            ));
        }
        if !functions.insert(function.sig.ident.to_string()) {
            return Err("Carrion Howler function is duplicated".to_owned());
        }
        let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
            return Err("Carrion Howler function must return one struct literal".to_owned());
        };
        if value.rest.is_some() || !value.attrs.is_empty() {
            return Err("Carrion Howler struct literal must be complete".to_owned());
        }
        if !declarative_expr(&Expr::Struct(value.clone())) {
            let field = value
                .fields
                .iter()
                .find_map(|field| {
                    if declarative_expr(&field.expr) {
                        None
                    } else {
                        match &field.member {
                            syn::Member::Named(name) => Some(name.to_string()),
                            _ => Some("unnamed".to_owned()),
                        }
                    }
                })
                .unwrap_or_else(|| "nested literal".to_owned());
            return Err(format!(
                "Carrion Howler function {} contains an unsupported executable expression in {field}",
                function.sig.ident
            ));
        }
        if !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(_)))
        {
            return Err("Carrion Howler function signature differs".to_owned());
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "life_pump_ability".to_owned(),
            "life_pump_cost".to_owned(),
        ])
    {
        return Err(
            "Carrion Howler source must define identity, ability, and cost functions".to_owned(),
        );
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityCostBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "GeneralizedActivatedAbilityCost"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if imports != expected_imports {
        return Err("Carrion Howler imports differ from the literal source contract".to_owned());
    }
    let definition = candidate_carrion_howler::definition();
    if definition.id != CARD_ID
        || definition.name != "Carrion Howler"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(2)
        || definition.toughness != Some(2)
        || definition.is_basic_land
        || !definition.mana_colors.is_empty()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err(
            "Carrion Howler identity, {3}{B} payment, or 2/2 printed body differs".to_owned(),
        );
    }
    let ability = candidate_carrion_howler::life_pump_ability();
    if ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(0)
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
    {
        return Err("Carrion Howler activation identity or non-life costs differ".to_owned());
    }
    let cost = candidate_carrion_howler::life_pump_cost();
    if cost.card_definition != CARD_ID
        || cost.ability_id != ABILITY_ID
        || cost.cost
            != (GeneralizedActivatedAbilityCost {
                life_payment: 1,
                ..GeneralizedActivatedAbilityCost::default()
            })
    {
        return Err("Carrion Howler activation must pay exactly 1 life".to_owned());
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
                    matches!(names.as_slice(), [a,b] if (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "ManaCost" && (b == "new" || b == "with_colors")) || (a == "GeneralizedActivatedAbilityCost" && b == "default"))
                        || names.as_slice() == ["Some"]
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
