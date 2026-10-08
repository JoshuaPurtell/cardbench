//! Full-game RAV Dromad Purebred trace for its received-damage life trigger.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
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
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DROMAD-PUREBRED";
const ATTACKER_ID: &str = "RAV-WATCHWOLF";
const DAMAGE_SPELL_ID: &str = "RAV-CHAR";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dromad_purebred.rs";
const P0_DECK: &str = "rav_dromad_damage_life";
const P1_DECK: &str = "rav_char_dromad_damage";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dromad_purebred.rs"]
mod candidate_dromad_purebred;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dromad Purebred PvP result: {error}");
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
            eprintln!("Dromad Purebred PvP authority failed: {error}");
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
    let replay = run_match(seed, false)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.dromad != replay.dromad
        || captured.watchwolf != replay.watchwolf
        || captured.damage_spell != replay.damage_spell
        || captured.dromad_payment != replay.dromad_payment
        || captured.watchwolf_payment != replay.watchwolf_payment
        || captured.char_payment != replay.char_payment
        || captured.dromad_pt != replay.dromad_pt
        || captured.dromad_zone != replay.dromad_zone
        || captured.life_totals != replay.life_totals
    {
        return Err("Dromad Purebred full-game replay was not deterministic".to_owned());
    }
    let mutation_detected = binding_mutation_detected(seed)?;
    if !mutation_detected {
        return Err(
            "removing Dromad Purebred's damage-sized life-gain trigger escaped the trace"
                .to_owned(),
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
        "card": {"collector_number": 15, "definition_id": CARD_ID, "name": "Dromad Purebred"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and spell resolution",
                "119: life totals and life gain",
                "120: damage and damage events",
                "302.1-302.2: creature permanents and control",
                "510: combat damage",
                "601.2 and 608.3a: casting and resolving spells",
                "603.2 and 603.3: triggered abilities and putting them on the stack"
            ],
            "source_contract": "RAV Dromad Purebred identity and candidate-owned positive received-damage trigger",
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
            "received_damage_life_gain_mutation_detected": mutation_detected,
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
    dromad: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    damage_spell: Option<ObjectId>,
    dromad_payment: Vec<Color>,
    watchwolf_payment: Vec<Color>,
    char_payment: Vec<Color>,
    dromad_pt: Option<(i32, i32)>,
    dromad_zone: Option<Zone>,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    for seed in 0..128 {
        let captured = run_match(seed, false)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error)
                if error.contains("did not reach Dromad")
                    || error.contains("did not receive positive damage")
                    || error.contains("did not cast Char") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a Dromad damage-trigger trace".to_owned())
}

fn binding_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, true)?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("Dromad Purebred did not gain four life for four damage")
    }))
}

fn run_match(seed: u64, mutate_life_gain: bool) -> Result<CapturedMatch, String> {
    let dromad_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ATTACKER_ID.to_owned(),
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
    let char_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: DAMAGE_SPELL_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [(P0_DECK, &dromad_deck), (P1_DECK, &char_deck)] {
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
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Dromad Purebred is missing from the RAV definition registry")?;
    *registered = candidate_dromad_purebred::definition();

    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    let mut trigger = candidate_dromad_purebred::triggered_ability();
    if mutate_life_gain {
        trigger.ability.effects = vec![Effect::GainLifeController { amount: 1 }];
    }
    triggers.push(trigger);
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
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &dromad_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &char_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DromadPolicy::new(PlayerId(0), true)),
        Box::new(DromadPolicy::new(PlayerId(1), false)),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut dromad = None;
    let mut watchwolf = None;
    let mut damage_spell = None;
    let mut dromad_payment = Vec::new();
    let mut watchwolf_payment = Vec::new();
    let mut char_payment = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Dromad Purebred PvP match exceeded its development bound".to_owned());
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
            if let GameEvent::SpellManaPaid {
                player,
                card,
                colors,
            } = event
            {
                match game.card_definition(*card).map(|definition| definition.id) {
                    Ok(CARD_ID) if *player == PlayerId(0) => {
                        dromad = Some(*card);
                        dromad_payment = colors.clone();
                    }
                    Ok(ATTACKER_ID) if *player == PlayerId(0) => {
                        watchwolf = Some(*card);
                        watchwolf_payment = colors.clone();
                    }
                    Ok(DAMAGE_SPELL_ID) if *player == PlayerId(1) && damage_spell.is_none() => {
                        damage_spell = Some(*card);
                        char_payment = colors.clone();
                    }
                    _ => {}
                }
            }
            if let GameEvent::SpellCast { player, card } = event {
                if *player == PlayerId(0)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CARD_ID)
                {
                    dromad = Some(*card);
                }
                if *player == PlayerId(0)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == ATTACKER_ID)
                {
                    watchwolf = Some(*card);
                }
                if *player == PlayerId(1)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == DAMAGE_SPELL_ID)
                    && damage_spell.is_none()
                {
                    damage_spell = Some(*card);
                }
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let dromad_pt = dromad
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let dromad_zone = dromad.and_then(|card| game.zone_of(card));
    let life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
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
        dromad,
        watchwolf,
        damage_spell,
        dromad_payment,
        watchwolf_payment,
        char_payment,
        dromad_pt,
        dromad_zone,
        life_totals,
    })
}

struct DromadPolicy {
    player: PlayerId,
    pilot: bool,
    watchwolf_cast: bool,
    dromad_cast: bool,
    char_turn: Option<u32>,
}

impl DromadPolicy {
    fn new(player: PlayerId, pilot: bool) -> Self {
        Self {
            player,
            pilot,
            watchwolf_cast: false,
            dromad_cast: false,
            char_turn: None,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
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

    fn make_mana(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing_colored = cost
            .colored
            .iter()
            .copied()
            .find(|color| view.mana_pool.amount(*color) == 0);
        let total: u16 = Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum();
        let needed = missing_colored.or_else(|| {
            if total < u16::from(cost.generic) + u16::try_from(cost.colored.len()).ok()? {
                cost.colored.first().copied().or(Some(Color::White))
            } else {
                None
            }
        })?;
        let land = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))?;
        let color = match land.basic_land_type? {
            cardbench_magic_engine::BasicLandType::Plains => Color::White,
            cardbench_magic_engine::BasicLandType::Forest => Color::Green,
            cardbench_magic_engine::BasicLandType::Mountain => Color::Red,
            _ => return None,
        };
        if missing_colored.is_some_and(|required| required != color) {
            let matching = view.own_battlefield.iter().find(|card| {
                if card.tapped || !card.card_types.contains(&CardType::Land) {
                    return false;
                }
                matches!(
                    (card.basic_land_type, needed),
                    (
                        Some(cardbench_magic_engine::BasicLandType::Plains),
                        Color::White
                    ) | (
                        Some(cardbench_magic_engine::BasicLandType::Forest),
                        Color::Green
                    ) | (
                        Some(cardbench_magic_engine::BasicLandType::Mountain),
                        Color::Red
                    )
                )
            })?;
            let produced = match matching.basic_land_type? {
                cardbench_magic_engine::BasicLandType::Plains => Color::White,
                cardbench_magic_engine::BasicLandType::Forest => Color::Green,
                cardbench_magic_engine::BasicLandType::Mountain => Color::Red,
                _ => return None,
            };
            return Some(PolicyAction::ActivateManaAbility {
                land: matching.id,
                color: produced,
            });
        }
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
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

    fn cast_action(
        &self,
        view: &GameView,
        definition: &str,
        cost: ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = Self::payment(view, &cost) {
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        self.make_mana(view, &cost)
    }
}

impl CodePolicy for DromadPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.dromad-damage-life.v1"
        } else {
            "rav.char-dromad-damage.v1"
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
            let attackers = if self.pilot {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(ATTACKER_ID) && card.can_attack)
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
        if self.pilot {
            if !self.watchwolf_cast {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(action) = self.cast_action(view, ATTACKER_ID, cost, vec![]) {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.watchwolf_cast = true;
                    }
                    return action;
                }
            }
            if !self.dromad_cast {
                let cost = ManaCost::with_colors(4, [Color::White]);
                if let Some(action) = self.cast_action(view, CARD_ID, cost, vec![]) {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.dromad_cast = true;
                    }
                    return action;
                }
            }
        } else if self.char_turn != Some(view.turn) {
            if let Some(target) = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(2, [Color::Red]);
                if let Some(action) = self.cast_action(
                    view,
                    DAMAGE_SPELL_ID,
                    cost,
                    vec![Target::Permanent(target.id)],
                ) {
                    if matches!(action, PolicyAction::CastWithPayment { .. }) {
                        self.char_turn = Some(view.turn);
                    }
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let dromad = captured.dromad.ok_or("Dromad Purebred was not cast")?;
    let watchwolf = captured.watchwolf.ok_or("Watchwolf was not cast")?;
    let char = captured.damage_spell.ok_or("Dromad did not cast Char")?;
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Dromad Purebred trace did not reach a terminal player-zero win".to_owned());
    }
    if captured.dromad_pt != Some((1, 5)) || captured.dromad_zone != Some(Zone::Battlefield) {
        return Err(format!(
            "Dromad did not survive the positive damage as a live 1/5: P/T {:?}, zone {:?}",
            captured.dromad_pt, captured.dromad_zone
        ));
    }
    if captured.watchwolf_payment.len() != 2
        || !captured.watchwolf_payment.contains(&Color::Green)
        || !captured.watchwolf_payment.contains(&Color::White)
    {
        return Err(format!(
            "Watchwolf must be paid with green and white mana: {:?}",
            captured.watchwolf_payment
        ));
    }
    if captured.dromad_payment.len() != 5 || !captured.dromad_payment.contains(&Color::White) {
        return Err(format!(
            "Dromad Purebred must be paid with {{4}}{{W}}: {:?}",
            captured.dromad_payment
        ));
    }
    if captured.char_payment.len() != 3 || !captured.char_payment.contains(&Color::Red) {
        return Err(format!(
            "Char must be paid with {{2}}{{R}}: {:?}",
            captured.char_payment
        ));
    }

    let wolf_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf)).ok_or("Watchwolf cast receipt missing")?;
    let wolf_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf resolution receipt missing")?;
    let dromad_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == dromad)).ok_or("Dromad spell cast receipt missing")?;
    let dromad_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == dromad),
    )
    .ok_or("Dromad resolution receipt missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 } if *source == char && *permanent == dromad))
        .ok_or("Dromad did not receive positive damage from Char")?;
    let char_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == char)).ok_or("Char spell cast receipt missing")?;
    let char_resolved = event_index_after(
        &captured.events,
        char_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == char),
    )
    .ok_or("Char did not resolve")?;
    let char_moved = event_index_after(&captured.events, char_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == char)).ok_or("resolved Char did not move to its graveyard")?;
    let trigger = event_index_after(&captured.events, damage, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: "damage-gain-life-from-source-damage", .. } if *source == dromad))
        .ok_or("Dromad received-damage trigger did not reach the stack")?;
    let ability_resolved = event_index_after(&captured.events, trigger, |event| matches!(event, GameEvent::AbilityResolved { source, ability: "damage-gain-life-from-source-damage", .. } if *source == dromad))
        .ok_or("Dromad received-damage trigger did not resolve")?;
    let life_gain = event_index_after(&captured.events, trigger, |event| {
        matches!(
            event,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount: 4
            }
        )
    })
    .ok_or("Dromad Purebred did not gain four life for four damage")?;
    let attackers = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("Watchwolf attack receipt missing")?;
    let combat_damage = event_index_after(&captured.events, attackers, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watchwolf)).ok_or("Watchwolf combat damage receipt missing")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal player-zero game event missing")?;
    if !(wolf_cast < wolf_resolved
        && dromad_cast < dromad_resolved
        && dromad_resolved < damage
        && char_cast < damage
        && damage < char_resolved
        && char_resolved < char_moved
        && char_moved < trigger
        && damage < trigger
        && trigger < life_gain
        && life_gain < ability_resolved
        && trigger < ability_resolved
        && attackers < combat_damage
        && wolf_resolved < attackers
        && combat_damage < ended
        && life_gain < ended)
    {
        return Err(format!(
            "Dromad cast, damage, trigger, life, combat, and terminal event order differs (watchwolf={wolf_cast}/{wolf_resolved}, Dromad={dromad_cast}/{dromad_resolved}, Char={char_cast}/{damage}/{char_resolved}/{char_moved}, trigger={trigger}/{life_gain}/{ability_resolved}, combat={attackers}/{combat_damage}, end={ended})"
        ));
    }

    let events = relevant_events(&captured.events, dromad, watchwolf, char);
    let mut ordered_event_sequence = vec![
        wolf_cast,
        wolf_resolved,
        dromad_cast,
        dromad_resolved,
        char_cast,
        damage,
        char_resolved,
        char_moved,
        trigger,
        life_gain,
        ability_resolved,
        attackers,
        combat_damage,
        ended,
    ];
    ordered_event_sequence.sort_unstable();
    Ok(json!({
        "id": "rav-dromad-purebred-received-damage-life-gain",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.dromad-damage-life.v1", "rav.char-dromad-damage.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "life_at_match_end": captured.life_totals,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "dromad_object": dromad.0,
        "watchwolf_object": watchwolf.0,
        "damage_spell_object": char.0,
        "dromad_power_toughness_after_damage": {"power": captured.dromad_pt.map(|pt| pt.0), "toughness": captured.dromad_pt.map(|pt| pt.1)},
        "dromad_zone_after_damage": format!("{:?}", captured.dromad_zone),
        "dromad_cast_payment": mana_payment_json(&captured.dromad_payment, 4, Color::White),
        "watchwolf_cast_payment": mana_payment_json(&captured.watchwolf_payment, 0, Color::Green),
        "char_cast_payment": mana_payment_json(&captured.char_payment, 2, Color::Red),
        "ordered_event_sequence": ordered_event_sequence,
        "event_positions": {
            "watchwolf_cast": wolf_cast,
            "watchwolf_resolved": wolf_resolved,
            "dromad_cast": dromad_cast,
            "dromad_resolved": dromad_resolved,
            "char_cast": char_cast,
            "dromad_damage": damage,
            "char_resolved": char_resolved,
            "char_graveyard_move": char_moved,
            "trigger_stacked": trigger,
            "life_gained": life_gain,
            "trigger_resolved": ability_resolved,
            "watchwolf_attack": attackers,
            "combat_damage": combat_damage,
            "terminal": ended
        },
        "events": events,
        "passed": true
    }))
}

fn mana_payment_json(spent: &[Color], generic: u8, colored: Color) -> Value {
    json!({
        "spent": spent.iter().map(|color| format!("{:?}", color)).collect::<Vec<_>>(),
        "generic_symbols": generic,
        "required_colored_symbol": format!("{:?}", colored),
        "paid_colored_symbol": spent.contains(&colored),
        "total_symbols": spent.len()
    })
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn relevant_events(
    events: &[GameEvent],
    dromad: ObjectId,
    watchwolf: ObjectId,
    char: ObjectId,
) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid" | "SpellCast" | "SpellResolved" => event
                .facts
                .objects
                .iter()
                .any(|object| [dromad.0, watchwolf.0, char.0].contains(&object.0)),
            "CardMoved" => event
                .facts
                .objects
                .iter()
                .any(|object| [dromad.0, watchwolf.0, char.0].contains(&object.0)),
            "DamageDealtToPermanent" => {
                event
                    .facts
                    .objects
                    .contains(&cardbench_magic_protocol::ObjectRef(char.0))
                    || event
                        .facts
                        .objects
                        .contains(&cardbench_magic_protocol::ObjectRef(dromad.0))
            }
            "TriggeredAbilityStacked" | "AbilityResolved" => event
                .facts
                .objects
                .contains(&cardbench_magic_protocol::ObjectRef(dromad.0)),
            "LifeGained" => event.facts.seats.iter().any(|seat| seat.0 == 0),
            "AttackersDeclared" => event
                .facts
                .objects
                .contains(&cardbench_magic_protocol::ObjectRef(watchwolf.0)),
            "DamageDealtToPlayer" => event
                .facts
                .objects
                .contains(&cardbench_magic_protocol::ObjectRef(watchwolf.0)),
            "GameEnded" => true,
            _ => false,
        })
        .collect()
}

fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for event in events {
        for byte in format!("{event:?}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => definition_id,
        _ => "",
    };
    let passed = definition_id == CARD_ID;
    json!({
        "collector_number": card.collector_number,
        "definition_id": definition_id,
        "name": card.name,
        "engine_status": "executable_compatibility_slice",
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false
    })
}

fn validate_candidate_implementation() -> Result<(), String> {
    let candidate_definition = candidate_dromad_purebred::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Dromad Purebred is missing from the executable RAV registry")?;
    if candidate_definition != registered_definition {
        return Err(
            "candidate Dromad definition differs from the candidate-owned RAV registry binding"
                .to_owned(),
        );
    }
    let candidate_trigger = candidate_dromad_purebred::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Dromad Purebred received-damage trigger is missing from the RAV registry")?;
    if candidate_trigger != registered_trigger {
        return Err(
            "candidate Dromad trigger differs from its candidate-owned RAV registry binding"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Dromad Purebred source: {error}"))?;
    if source.len() > 64 * 1024 {
        return Err("Dromad Purebred source exceeds 64 KiB".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Dromad Purebred source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Dromad Purebred source has a non-documentation module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Dromad Purebred source contains an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(function) if function.attrs.iter().all(is_safe_function_attribute) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) {
                    return Err("Dromad Purebred source has duplicate functions".to_owned());
                }
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => {
                        return Err(
                            "Dromad Purebred source contains an unexpected function".to_owned()
                        );
                    }
                };
                validate_candidate_function(function, expected_type)?;
                let expected_struct = expected_type;
                if function.block.stmts.len() != 1 {
                    return Err(
                        "Dromad Purebred functions must be direct declarative literals".to_owned(),
                    );
                }
                let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
                    return Err("Dromad Purebred functions cannot contain statements".to_owned());
                };
                validate_literal_shape(expression, expected_struct)?;
                if !safe_candidate_expression(expression) {
                    if let Expr::Struct(structure) = expression {
                        if let Some(field) = structure
                            .fields
                            .iter()
                            .find(|field| !safe_candidate_expression(&field.expr))
                        {
                            let field_name = match &field.member {
                                syn::Member::Named(name) => name.to_string(),
                                _ => "<unnamed>".to_owned(),
                            };
                            return Err(format!(
                                "Dromad Purebred source contains an executable expression in field {field_name}"
                            ));
                        }
                    }
                    return Err(
                        "Dromad Purebred source contains an executable expression".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "Dromad Purebred source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Dromad Purebred imports or module structure differ from the task boundary".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_function(function: &syn::ItemFn, return_type: &str) -> Result<(), String> {
    let signature = &function.sig;
    let visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    let returns_expected = matches!(&signature.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(return_type)));
    if !visibility
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !returns_expected
    {
        return Err(format!(
            "Dromad Purebred function must be a plain {}() -> {}",
            signature.ident, return_type
        ));
    }
    Ok(())
}

fn validate_literal_shape(expression: &Expr, expected: &str) -> Result<(), String> {
    let Expr::Struct(structure) = expression else {
        return Err(format!(
            "Dromad Purebred {expected} must return a struct literal"
        ));
    };
    if structure
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != expected)
        || structure.rest.is_some()
    {
        return Err(format!("Dromad Purebred literal must be {expected}"));
    }
    let expected_fields: BTreeSet<&str> = match expected {
        "CardDefinition" => [
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
        .collect(),
        "TriggeredAbilityBinding" => ["card_definition", "ability"].into_iter().collect(),
        _ => return Err("unexpected candidate literal type".to_owned()),
    };
    let actual_fields: BTreeSet<String> = structure
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    if actual_fields.len() != structure.fields.len()
        || actual_fields
            != expected_fields
                .iter()
                .map(|field| (*field).to_owned())
                .collect()
    {
        return Err(format!(
            "Dromad Purebred {expected} fields differ from the declared type"
        ));
    }
    Ok(())
}

fn safe_candidate_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => path.attrs.is_empty() && path.qself.is_none(),
        Expr::Lit(literal) => {
            literal.attrs.is_empty()
                && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_))
        }
        Expr::Array(array) => {
            array.attrs.is_empty() && array.elems.iter().all(safe_candidate_expression)
        }
        Expr::Reference(reference) => {
            reference.attrs.is_empty() && safe_candidate_expression(&reference.expr)
        }
        Expr::Paren(paren) => paren.attrs.is_empty() && safe_candidate_expression(&paren.expr),
        Expr::Group(group) => group.attrs.is_empty() && safe_candidate_expression(&group.expr),
        Expr::Call(call) => {
            if !call.attrs.is_empty() {
                return false;
            }
            let Expr::Path(function) = &*call.func else {
                return false;
            };
            let path = function
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            matches!(
                path.as_str(),
                "BTreeSet::from"
                    | "BTreeSet::new"
                    | "ManaCost::with_colors"
                    | "ManaCost::new"
                    | "Some"
            ) && call.args.iter().all(safe_candidate_expression)
        }
        Expr::Struct(structure) => {
            if !structure.attrs.is_empty() {
                return false;
            }
            let name = structure
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            matches!(
                name.as_deref(),
                Some(
                    "CardDefinition"
                        | "TriggeredAbilityBinding"
                        | "TriggeredAbility"
                        | "GainLifeController"
                )
            ) && structure.rest.is_none()
                && structure
                    .fields
                    .iter()
                    .all(|field| field.attrs.is_empty() && safe_candidate_expression(&field.expr))
        }
        Expr::Macro(mac) => {
            if !mac.attrs.is_empty() {
                return false;
            }
            if !mac.mac.path.is_ident("vec") {
                return false;
            }
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|expressions| expressions.iter().all(safe_candidate_expression))
        }
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(&literal.lit, Lit::Str(_))))
}

fn is_safe_function_attribute(attribute: &syn::Attribute) -> bool {
    is_doc_attribute(attribute)
        || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
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
        UseTree::Glob(_) | UseTree::Rename(_) => false,
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), b"0.0\n").map_err(|error| error.to_string())?;
    if let Some(trace) = report["traces"]
        .as_array()
        .and_then(|traces| traces.first())
    {
        std::fs::write(
            root.join("trace.json"),
            serde_json::to_vec_pretty(trace).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}
