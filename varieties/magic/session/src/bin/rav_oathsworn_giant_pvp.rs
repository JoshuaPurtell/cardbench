//! Full-game RAV Oathsworn Giant trace for its other-creature +0/+2 and vigilance effects.

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-OATHSWORN-GIANT";
const OTHER_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/oathsworn_giant.rs";
const P0_DECK: &str = "rav_oathsworn_giant_static_source";
const P1_DECK: &str = "rav_oathsworn_giant_land_only";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/oathsworn_giant.rs"]
mod candidate_oathsworn_giant;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Oathsworn Giant PvP result: {error}");
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
            eprintln!("Oathsworn Giant PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_implementation()?;
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
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.giant != replay.giant
        || captured.watchwolf != replay.watchwolf
        || captured.watchwolf_base_pt != replay.watchwolf_base_pt
        || captured.watchwolf_buffed_pt != replay.watchwolf_buffed_pt
        || captured.giant_pt != replay.giant_pt
        || captured.watchwolf_vigilance != replay.watchwolf_vigilance
        || captured.giant_vigilance != replay.giant_vigilance
        || captured.watchwolf_tapped_after_combat != replay.watchwolf_tapped_after_combat
        || captured.giant_tapped_after_combat != replay.giant_tapped_after_combat
    {
        return Err("Oathsworn Giant full-game replay was not deterministic".to_owned());
    }
    let toughness_mutation_detected = binding_toughness_mutation_detected(seed)?;
    let vigilance_mutation_detected = binding_vigilance_mutation_detected(seed)?;
    let printed_vigilance_mutation_detected = printed_vigilance_mutation_detected(seed)?;
    if !toughness_mutation_detected
        || !vigilance_mutation_detected
        || !printed_vigilance_mutation_detected
    {
        return Err("an Oathsworn Giant semantic mutation escaped the full-game checks".to_owned());
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
        "card": {"collector_number": 27, "definition_id": CARD_ID, "name": "Oathsworn Giant"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: white identity and colored payment",
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and spell resolution",
                "208.1 and 613.4c: power/toughness characteristics and continuous effects",
                "302.1-302.2: creature permanents and control",
                "508 and 510: attackers and combat damage",
                "601.2 and 608.3a: casting and resolving creature spells"
            ],
            "source_contract": "RAV Oathsworn Giant identity, printed vigilance, and candidate-owned +0/+2 and vigilance effects for other controlled creatures",
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
            "other_creature_toughness_bonus_mutation_detected": toughness_mutation_detected,
            "granted_vigilance_mutation_detected": vigilance_mutation_detected,
            "printed_vigilance_mutation_detected": printed_vigilance_mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    giant: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    giant_cast_payment: Vec<Color>,
    watchwolf_cast_payment: Vec<Color>,
    watchwolf_base_pt: Option<(i32, i32)>,
    watchwolf_buffed_pt: Option<(i32, i32)>,
    giant_pt: Option<(i32, i32)>,
    watchwolf_vigilance: Option<bool>,
    giant_vigilance: Option<bool>,
    watchwolf_tapped_after_combat: Option<bool>,
    giant_tapped_after_combat: Option<bool>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let seed = 0;
    let captured = run_match(seed, Mutation::None)?;
    validate_trace(&captured, seed)?;
    Ok((seed, captured))
}

fn binding_toughness_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, Mutation::WrongToughness)?;
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains("Watchwolf live power/toughness")))
}

fn binding_vigilance_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, Mutation::MissingGrantedVigilance)?;
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains("Watchwolf did not gain vigilance")))
}

fn printed_vigilance_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, Mutation::MissingPrintedVigilance)?;
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains("Oathsworn Giant must retain printed vigilance")))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongToughness,
    MissingGrantedVigilance,
    MissingPrintedVigilance,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let giant_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: OTHER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let land_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [(P0_DECK, &giant_deck), (P1_DECK, &land_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut static_bindings = rav_static_continuous_effect_bindings();
    if mutation == Mutation::WrongToughness {
        let binding = static_bindings
            .iter_mut()
            .find(|binding| {
                binding.card_definition == CARD_ID
                    && matches!(
                        binding.change,
                        ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                    )
            })
            .ok_or("Oathsworn Giant static binding is missing from the registry")?;
        binding.change = ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
            power: 0,
            toughness: 0,
        };
    }
    if mutation == Mutation::MissingGrantedVigilance {
        static_bindings.retain(|binding| {
            !(binding.card_definition == CARD_ID
                && binding.change
                    == ContinuousChange::OtherControlledCreaturesAddKeyword(Keyword::Vigilance))
        });
    }
    let mut definitions = card_definitions();
    if mutation == Mutation::MissingPrintedVigilance {
        let definition = definitions
            .iter_mut()
            .find(|definition| definition.id == CARD_ID)
            .ok_or("Oathsworn Giant definition is missing from the registry")?;
        definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Vigilance);
    }
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        static_bindings,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &giant_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &land_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GiantPolicy::new(PlayerId(0), true)),
        Box::new(GiantPolicy::new(PlayerId(1), false)),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut giant = None;
    let mut watchwolf = None;
    let mut giant_cast_payment = Vec::new();
    let mut watchwolf_cast_payment = Vec::new();
    let mut watchwolf_base_pt = None;
    let mut watchwolf_buffed_pt = None;
    let mut giant_pt = None;
    let mut watchwolf_vigilance = None;
    let mut giant_vigilance = None;
    let mut watchwolf_tapped_after_combat = None;
    let mut giant_tapped_after_combat = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Oathsworn Giant PvP match exceeded its development bound".to_owned());
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
        let before = game.event_log.len();
        attempted_moves += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => accepted_moves += 1,
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } => match game.card_definition(*card).map(|definition| definition.id) {
                    Ok(CARD_ID) => giant = Some(*card),
                    Ok(OTHER_ID) => watchwolf = Some(*card),
                    _ => {}
                },
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } => match game.card_definition(*card).map(|definition| definition.id) {
                    Ok(CARD_ID) => {
                        giant = Some(*card);
                        giant_cast_payment = colors.clone();
                    }
                    Ok(OTHER_ID) => {
                        watchwolf = Some(*card);
                        watchwolf_cast_payment = colors.clone();
                    }
                    _ => {}
                },
                GameEvent::SpellResolved { card } => {
                    if watchwolf == Some(*card) && watchwolf_base_pt.is_none() {
                        watchwolf_base_pt = game.characteristics(*card).ok().and_then(pt);
                    }
                    if giant == Some(*card) {
                        let characteristics = game.characteristics(*card).ok();
                        giant_pt = characteristics.clone().and_then(pt);
                        giant_vigilance = characteristics
                            .map(|value| value.keywords.contains(&Keyword::Vigilance));
                        if let Some(watchwolf) = watchwolf {
                            let characteristics = game.characteristics(watchwolf).ok();
                            watchwolf_buffed_pt = characteristics.clone().and_then(pt);
                            watchwolf_vigilance = characteristics
                                .map(|value| value.keywords.contains(&Keyword::Vigilance));
                        }
                    }
                }
                _ => {}
            }
        }
        if watchwolf_tapped_after_combat.is_none()
            && game.event_log[before..].iter().any(|event| {
                matches!(event,
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. }
                    if Some(*source) == watchwolf || Some(*source) == giant)
            })
        {
            watchwolf_tapped_after_combat =
                watchwolf.and_then(|card| game.object(card).ok().map(|object| object.tapped));
            giant_tapped_after_combat =
                giant.and_then(|card| game.object(card).ok().map(|object| object.tapped));
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        giant,
        watchwolf,
        giant_cast_payment,
        watchwolf_cast_payment,
        watchwolf_base_pt,
        watchwolf_buffed_pt,
        giant_pt,
        watchwolf_vigilance,
        giant_vigilance,
        watchwolf_tapped_after_combat,
        giant_tapped_after_combat,
    })
}

fn pt(value: cardbench_magic_engine::Characteristics) -> Option<(i32, i32)> {
    Some((value.power?, value.toughness?))
}

struct GiantPolicy {
    player: PlayerId,
    pilot: bool,
    watchwolf_cast: bool,
    giant_cast: bool,
}

impl GiantPolicy {
    fn new(player: PlayerId, pilot: bool) -> Self {
        Self {
            player,
            pilot,
            watchwolf_cast: false,
            giant_cast: false,
        }
    }

    fn payment(view: &GameView, cost: ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
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
        let mut generic = Vec::with_capacity(usize::from(cost.generic));
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

    fn make_mana(&self, view: &GameView, cost: ManaCost) -> Option<PolicyAction> {
        let mut need = None;
        for color in &cost.colored {
            if view.mana_pool.amount(*color) == 0 {
                need = Some(*color);
                break;
            }
        }
        let total: u16 = Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum();
        let missing_colored = need.is_some();
        if need.is_none() && total < u16::from(cost.generic) + cost.colored.len() as u16 {
            need = Some(Color::White);
        }
        let color = need?;
        let land = view.own_battlefield.iter().find(|card| {
            if card.tapped || card.card_types.contains(&CardType::Creature) {
                return false;
            }
            match (card.basic_land_type, color, missing_colored) {
                (Some(cardbench_magic_engine::BasicLandType::Plains), Color::White, true) => true,
                (Some(cardbench_magic_engine::BasicLandType::Forest), Color::Green, true) => true,
                (
                    Some(
                        cardbench_magic_engine::BasicLandType::Plains
                        | cardbench_magic_engine::BasicLandType::Forest,
                    ),
                    _,
                    false,
                ) => true,
                _ => false,
            }
        })?;
        let produced = match land.basic_land_type? {
            cardbench_magic_engine::BasicLandType::Plains => Color::White,
            cardbench_magic_engine::BasicLandType::Forest => Color::Green,
            _ => return None,
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: produced,
        })
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for GiantPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.oathsworn-giant-static-source.v1"
        } else {
            "rav.oathsworn-giant-passive-opponent.v1"
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
            let both_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
                && view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(OTHER_ID));
            let attackers = if self.pilot && both_live {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.pilot && !self.watchwolf_cast {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(OTHER_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(mana_selection) = Self::payment(view, cost.clone()) {
                    self.watchwolf_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = self.make_mana(view, cost) {
                    return action;
                }
            }
        }
        let watchwolf_live = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(OTHER_ID));
        if self.pilot && watchwolf_live && !self.giant_cast {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(4, [Color::White, Color::White]);
                if let Some(mana_selection) = Self::payment(view, cost.clone()) {
                    self.giant_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = self.make_mana(view, cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let giant = captured.giant.ok_or("Oathsworn Giant was not cast")?;
    let watchwolf = captured
        .watchwolf
        .ok_or("an allied Watchwolf was not cast")?;
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Oathsworn Giant PvP trace did not reach a clean terminal seat-zero win".to_owned(),
        );
    }
    if captured.watchwolf_base_pt != Some((3, 3))
        || captured.watchwolf_buffed_pt != Some((3, 5))
        || captured.giant_pt != Some((3, 4))
    {
        return Err(format!(
            "Oathsworn Giant Watchwolf live power/toughness or source self-exclusion differs (Watchwolf base {:?}, buffed {:?}, Giant {:?})",
            captured.watchwolf_base_pt, captured.watchwolf_buffed_pt, captured.giant_pt
        ));
    }
    if captured.watchwolf_cast_payment.len() != 2
        || !captured.watchwolf_cast_payment.contains(&Color::Green)
        || !captured.watchwolf_cast_payment.contains(&Color::White)
    {
        return Err(format!(
            "Watchwolf must pay green and white mana: {:?}",
            captured.watchwolf_cast_payment
        ));
    }
    if captured.giant_cast_payment.len() != 6
        || captured
            .giant_cast_payment
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            < 2
    {
        return Err(format!(
            "Oathsworn Giant must pay two white and four generic mana: {:?}",
            captured.giant_cast_payment
        ));
    }
    if captured.watchwolf_vigilance != Some(true) {
        return Err("Watchwolf did not gain vigilance from Oathsworn Giant".to_owned());
    }
    if captured.giant_vigilance != Some(true) {
        return Err("Oathsworn Giant must retain printed vigilance".to_owned());
    }
    if captured.watchwolf_tapped_after_combat != Some(false)
        || captured.giant_tapped_after_combat != Some(false)
    {
        return Err(format!(
            "vigilance attackers should remain untapped after combat (Watchwolf {:?}, Oathsworn Giant {:?})",
            captured.watchwolf_tapped_after_combat, captured.giant_tapped_after_combat
        ));
    }
    let spell_order = spell_order(&captured.events, giant, watchwolf)?;
    let attack = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf) && attackers.contains(&giant))).ok_or("the buffed Watchwolf and Giant did not attack together")?;
    let blockers = event_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("empty opposing blockers receipt missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watchwolf)).ok_or("buffed Watchwolf combat damage receipt missing")?;
    let giant_damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == giant)).ok_or("Oathsworn Giant combat damage receipt missing")?;
    let end = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal winner event missing")?;
    if !(spell_order.0 < attack
        && attack < blockers
        && blockers < damage
        && damage < giant_damage
        && giant_damage < end)
    {
        return Err(
            "Oathsworn Giant resolution, buffed attack, damage, and terminal event order differs"
                .to_owned(),
        );
    }
    let events = relevant_events(&captured.events, giant, watchwolf);
    let kinds: Vec<_> = events.iter().map(|event| event.kind.as_str()).collect();
    let required = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    if !contains_ordered_subsequence(&kinds, &required) {
        return Err(format!(
            "Oathsworn Giant ordered card/combat events differ: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-oathsworn-giant-other-creature-plus-zero-two-and-vigilance",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.oathsworn-giant-static-source.v1", "rav.oathsworn-giant-passive-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "oathsworn_giant_object": giant.0,
        "other_controlled_creature_object": watchwolf.0,
        "giant_cast_payment": {"required": {"white_symbols": 2, "generic_symbols": 4, "total_symbols": 6}, "spent": {"white": captured.giant_cast_payment.iter().filter(|color| **color == Color::White).count(), "total": captured.giant_cast_payment.len()}},
        "watchwolf_cast_payment": {"required": {"green_symbols": 1, "white_symbols": 1, "total_symbols": 2}, "spent": {"green": captured.watchwolf_cast_payment.iter().filter(|color| **color == Color::Green).count(), "white": captured.watchwolf_cast_payment.iter().filter(|color| **color == Color::White).count(), "total": captured.watchwolf_cast_payment.len()}},
        "other_creature_base_power_toughness": {"power": 3, "toughness": 3},
        "other_creature_buffed_power_toughness": {"power": captured.watchwolf_buffed_pt.unwrap().0, "toughness": captured.watchwolf_buffed_pt.unwrap().1},
        "giant_power_toughness": {"power": captured.giant_pt.unwrap().0, "toughness": captured.giant_pt.unwrap().1},
        "static_bonus": {"power": 0, "toughness": 2, "applies_to_other_controlled_creatures": true, "applies_to_source": false},
        "granted_keyword": "Vigilance",
        "printed_keyword": "Vigilance",
        "watchwolf_remained_untapped_after_combat": captured.watchwolf_tapped_after_combat == Some(false),
        "oathsworn_giant_remained_untapped_after_combat": captured.giant_tapped_after_combat == Some(false),
        "events": events,
        "passed": true
    }))
}

fn spell_order(
    events: &[GameEvent],
    giant: ObjectId,
    watchwolf: ObjectId,
) -> Result<(usize, usize), String> {
    let watchwolf_resolved = event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf resolution missing")?;
    let giant_resolved = event_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == giant),
    )
    .ok_or("Oathsworn Giant resolution missing")?;
    let watchwolf_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf)).ok_or("Watchwolf cast receipt missing")?;
    let giant_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == giant)).ok_or("Oathsworn Giant cast receipt missing")?;
    if !(watchwolf_cast < watchwolf_resolved
        && watchwolf_resolved < giant_cast
        && giant_cast < giant_resolved)
    {
        return Err("Watchwolf must resolve before Oathsworn Giant's static buff".to_owned());
    }
    Ok((giant_resolved, watchwolf_resolved))
}

fn relevant_events(
    events: &[GameEvent],
    giant: ObjectId,
    watchwolf: ObjectId,
) -> Vec<ProjectedEvent> {
    let projected = project_events(events);
    let mut attack_seen = false;
    let mut blockers_seen = false;
    let mut watchwolf_damage_seen = false;
    let mut giant_damage_seen = false;
    let mut end_seen = false;
    projected
        .into_iter()
        .filter(|event| {
            if event.kind == "GameEnded" && !end_seen {
                end_seen = true;
                return true;
            }
            if event.kind == "AttackersDeclared"
                && !attack_seen
                && event
                    .canonical
                    .contains(&format!("ObjectId({})", watchwolf.0))
            {
                attack_seen = true;
                return true;
            }
            if event.kind == "BlockersDeclared"
                && !blockers_seen
                && event.canonical.contains("assignments: []")
            {
                blockers_seen = true;
                return true;
            }
            if event.kind == "DamageDealtToPlayer"
                && !watchwolf_damage_seen
                && event
                    .canonical
                    .contains(&format!("source: ObjectId({})", watchwolf.0))
            {
                watchwolf_damage_seen = true;
                return true;
            }
            if event.kind == "DamageDealtToPlayer"
                && !giant_damage_seen
                && event
                    .canonical
                    .contains(&format!("source: ObjectId({})", giant.0))
            {
                giant_damage_seen = true;
                return true;
            }
            if ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"]
                .contains(&event.kind.as_str())
            {
                let object = format!("card: ObjectId({})", giant.0);
                let other = format!("card: ObjectId({})", watchwolf.0);
                return (event.canonical.contains(&object) || event.canonical.contains(&other))
                    && (event.kind != "CardMoved" || event.canonical.contains("to: Battlefield"));
            }
            false
        })
        .collect()
}

fn contains_ordered_subsequence(actual: &[&str], expected: &[&str]) -> bool {
    let mut next = 0;
    for value in actual {
        if expected.get(next) == Some(value) {
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
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
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
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("reward.txt"),
        format!("{:.8}\n", report["harbor_reward"].as_f64().unwrap_or(0.0)),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_candidate_implementation() -> Result<(), String> {
    let submitted = candidate_oathsworn_giant::definition();
    let registry = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Oathsworn Giant is missing from the RAV registry")?;
    if submitted != registry
        || submitted.id != CARD_ID
        || submitted.name != "Oathsworn Giant"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(4, [Color::White, Color::White])
        || submitted.colors != BTreeSet::from([Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(3)
        || submitted.toughness != Some(4)
        || submitted.keywords != vec![Keyword::Vigilance]
        || !submitted.effects.is_empty()
    {
        return Err("Oathsworn Giant printed identity differs".to_owned());
    }
    let candidate_bindings = candidate_oathsworn_giant::static_continuous_effects();
    let toughness_binding = &candidate_bindings[0];
    let vigilance_binding = &candidate_bindings[1];
    let valid_toughness_change = toughness_binding.card_definition == CARD_ID
        && (toughness_binding.change
            == ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                power: 0,
                toughness: 0,
            }
            || toughness_binding.change
                == ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                    power: 0,
                    toughness: 2,
                });
    if !valid_toughness_change
        || vigilance_binding.card_definition != CARD_ID
        || vigilance_binding.change
            != ContinuousChange::OtherControlledCreaturesAddKeyword(Keyword::Vigilance)
    {
        return Err(
            "Oathsworn Giant candidate static bindings must include +0/+2 and granted vigilance"
                .to_owned(),
        );
    }
    let registered = rav_static_continuous_effect_bindings();
    if !candidate_bindings
        .iter()
        .all(|candidate| registered.contains(candidate))
    {
        return Err(
            "Oathsworn Giant's candidate-owned static bindings are not registered".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Oathsworn Giant source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Oathsworn Giant source is not declarative Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "StaticContinuousEffectBinding"],
    ]
    .into_iter()
    .map(|parts| parts.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                flatten_use(&[], &item.tree, &mut imports)?;
            }
            Item::Fn(function)
                if matches!(
                    function.sig.ident.to_string().as_str(),
                    "definition" | "static_continuous_effects"
                ) =>
            {
                if functions
                    .insert(function.sig.ident.to_string(), function)
                    .is_some()
                {
                    return Err("duplicate Oathsworn Giant function".to_owned());
                }
            }
            _ => {
                return Err(
                    "Oathsworn Giant module contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || functions.len() != 2 || file.items.len() != 4 {
        return Err(format!(
            "Oathsworn Giant module imports or item count differ ({} items)",
            file.items.len()
        ));
    }
    let definition = functions
        .get("definition")
        .ok_or("Oathsworn Giant definition() missing")?;
    let bindings = functions
        .get("static_continuous_effects")
        .ok_or("Oathsworn Giant static bindings function missing")?;
    validate_plain_function(definition, "definition", "CardDefinition")?;
    validate_static_bindings_function(bindings)?;
    let definition_fields = direct_struct_fields(definition, &["CardDefinition"])?;
    if definition_fields.len() != 13
        || !definition_fields
            .values()
            .all(|expression| safe_candidate_expr(expression))
    {
        return Err("Oathsworn Giant definition literal fields differ".to_owned());
    }
    let Stmt::Expr(Expr::Array(array), None) = &bindings.block.stmts[0] else {
        return Err("Oathsworn Giant static bindings must be a direct two-entry array".to_owned());
    };
    if array.attrs.len() != 0 || array.elems.len() != 2 {
        return Err(
            "Oathsworn Giant static bindings must have exactly two direct entries".to_owned(),
        );
    }
    let power_fields = binding_fields(&array.elems[0])?;
    if !is_string(power_fields["card_definition"], CARD_ID) {
        return Err(
            "Oathsworn Giant power/toughness binding has the wrong source identity".to_owned(),
        );
    }
    let change = direct_nested_struct_fields(
        power_fields["change"],
        &[
            "ContinuousChange",
            "OtherControlledCreaturesModifyPowerToughness",
        ],
    )?;
    if change.len() != 2
        || !check_int(change["power"], 0)
        || !(check_int(change["toughness"], 2) || check_int(change["toughness"], 0))
    {
        return Err("Oathsworn Giant source must encode a literal +0/+2 binding".to_owned());
    }
    let keyword_fields = binding_fields(&array.elems[1])?;
    if !is_string(keyword_fields["card_definition"], CARD_ID)
        || !is_granted_vigilance(keyword_fields["change"])
    {
        return Err(
            "Oathsworn Giant source must grant vigilance to other controlled creatures".to_owned(),
        );
    }
    if !array.elems.iter().all(safe_candidate_expr) {
        return Err(
            "Oathsworn Giant static bindings may contain only declarative literals".to_owned(),
        );
    }
    Ok(())
}

fn safe_candidate_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => literal.attrs.is_empty(),
        Expr::Path(path) => {
            path.attrs.is_empty()
                && path.qself.is_none()
                && [
                    &["Color", "White"][..],
                    &["CardType", "Creature"],
                    &["Keyword", "Vigilance"],
                ]
                .iter()
                .any(|expected| path_is(&path.path, expected))
        }
        Expr::Array(array) => array.attrs.is_empty() && array.elems.iter().all(safe_candidate_expr),
        Expr::Reference(reference) => {
            reference.attrs.is_empty()
                && reference.mutability.is_none()
                && safe_candidate_expr(&reference.expr)
        }
        Expr::Call(call) => {
            if !call.attrs.is_empty() || !call.args.iter().all(safe_candidate_expr) {
                return false;
            }
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            [
                &["BTreeSet", "from"][..],
                &["BTreeSet", "new"],
                &["ManaCost", "with_colors"],
                &["Some"],
                &["ContinuousChange", "OtherControlledCreaturesAddKeyword"],
            ]
            .iter()
            .any(|expected| path_is(&path.path, expected))
        }
        Expr::Struct(structure) => {
            if !structure.attrs.is_empty()
                || structure.rest.is_some()
                || ![
                    &["CardDefinition"][..],
                    &["StaticContinuousEffectBinding"],
                    &[
                        "ContinuousChange",
                        "OtherControlledCreaturesModifyPowerToughness",
                    ],
                ]
                .iter()
                .any(|expected| path_is(&structure.path, expected))
            {
                return false;
            }
            structure.fields.iter().all(|field| {
                field.attrs.is_empty()
                    && matches!(field.member, syn::Member::Named(_))
                    && field.colon_token.is_some()
                    && safe_candidate_expr(&field.expr)
            })
        }
        Expr::Macro(mac) => {
            if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
                return false;
            }
            let tokens = mac.mac.tokens.to_string().replace(' ', "");
            tokens.is_empty() || tokens == "Keyword::Vigilance"
        }
        _ => false,
    }
}

fn validate_static_bindings_function(function: &syn::ItemFn) -> Result<(), String> {
    let safe_attrs = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
            || attribute.path().is_ident("doc")
    });
    let crate_visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    let return_array = match &function.sig.output {
        syn::ReturnType::Type(_, ty) => match &**ty {
            Type::Array(array) => {
                type_is(&array.elem, &["StaticContinuousEffectBinding"]) && check_int(&array.len, 2)
            }
            _ => false,
        },
        syn::ReturnType::Default => false,
    };
    if !safe_attrs
        || !crate_visibility
        || function.sig.ident != "static_continuous_effects"
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !return_array
        || function.block.stmts.len() != 1
    {
        return Err(
            "Oathsworn Giant static_continuous_effects must return a literal two-entry array"
                .to_owned(),
        );
    }
    Ok(())
}

fn binding_fields<'a>(expr: &'a Expr) -> Result<BTreeMap<String, &'a Expr>, String> {
    let Expr::Struct(value) = expr else {
        return Err("Oathsworn Giant binding must be a direct struct".to_owned());
    };
    if !path_is(&value.path, &["StaticContinuousEffectBinding"])
        || value.rest.is_some()
        || !value.attrs.is_empty()
    {
        return Err("Oathsworn Giant binding literal differs".to_owned());
    }
    named_fields(&value.fields)
}

fn is_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(value) if matches!(&value.lit, Lit::Str(text) if text.value() == expected))
}

fn is_granted_vigilance(expr: &Expr) -> bool {
    matches!(expr, Expr::Call(call) if call.attrs.is_empty() && call.args.len() == 1
        && matches!(&*call.func, Expr::Path(path) if path_is(&path.path, &["ContinuousChange", "OtherControlledCreaturesAddKeyword"]))
        && matches!(call.args.first(), Some(Expr::Path(path)) if path_is(&path.path, &["Keyword", "Vigilance"])))
}

fn validate_plain_function(
    function: &syn::ItemFn,
    name: &str,
    return_type: &str,
) -> Result<(), String> {
    let safe_attrs = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
            || attribute.path().is_ident("doc")
    });
    let crate_visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    if !safe_attrs
        || !crate_visibility
        || function.sig.ident != name
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &[return_type]))
        || function.block.stmts.len() != 1
    {
        return Err(format!(
            "Oathsworn Giant {name} must be a plain declarative function"
        ));
    }
    Ok(())
}

fn direct_struct_fields<'a>(
    function: &'a syn::ItemFn,
    path: &[&str],
) -> Result<BTreeMap<String, &'a Expr>, String> {
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Oathsworn Giant must return one direct struct literal".to_owned());
    };
    if !path_is(&value.path, path) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Oathsworn Giant struct literal differs".to_owned());
    }
    named_fields(&value.fields)
}

fn direct_nested_struct_fields<'a>(
    expr: &'a Expr,
    path: &[&str],
) -> Result<BTreeMap<String, &'a Expr>, String> {
    let Expr::Struct(value) = expr else {
        return Err("Oathsworn Giant static change must be a direct enum struct".to_owned());
    };
    if !path_is(&value.path, path) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err("Oathsworn Giant static change differs".to_owned());
    }
    named_fields(&value.fields)
}

fn named_fields<'a>(
    fields: &'a syn::punctuated::Punctuated<syn::FieldValue, syn::token::Comma>,
) -> Result<BTreeMap<String, &'a Expr>, String> {
    let mut output = BTreeMap::new();
    for field in fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Oathsworn Giant fields must be named".to_owned());
        };
        if !field.attrs.is_empty()
            || field.colon_token.is_none()
            || output.insert(name.to_string(), &field.expr).is_some()
        {
            return Err("Oathsworn Giant field shorthand or duplicate".to_owned());
        }
    }
    Ok(output)
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
        _ => Err("Oathsworn Giant uses an unsupported import".to_owned()),
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
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}
fn check_int(expr: &Expr, expected: u64) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
}
