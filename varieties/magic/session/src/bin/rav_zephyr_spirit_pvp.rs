//! Full-game RAV Zephyr Spirit trace for its source-relative Blocks trigger.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TriggerCondition, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings, CardSemanticStatus, CatalogCard,
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
};
use cardbench_magic_session::project_events;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ZEPHYR-SPIRIT";
const ATTACKER_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WIN_CONDITION_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/zephyr_spirit.rs";
const P0_DECK: &str = "rav_zephyr_spirit_block_trigger";
const P1_DECK: &str = "rav_elves_of_deep_shadow_attack";
const TRIGGER_ID: &str = "blocks-return-source-owner-hand";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/zephyr_spirit.rs"]
mod candidate_zephyr_spirit;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Zephyr Spirit PvP result: {error}");
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
            eprintln!("Zephyr Spirit PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, false)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.zephyr != replay.zephyr
        || captured.attacker != replay.attacker
        || captured.watcher != replay.watcher
        || captured.zephyr_payment != replay.zephyr_payment
        || captured.attacker_payment != replay.attacker_payment
        || captured.watcher_pt != replay.watcher_pt
        || captured.zephyr_pt != replay.zephyr_pt
        || captured.zephyr_zone != replay.zephyr_zone
        || captured.life_totals != replay.life_totals
    {
        return Err("Zephyr Spirit terminal PvP replay was not deterministic".to_owned());
    }
    let mutation_detected = blocks_effect_mutation_detected(seed)?;
    if !mutation_detected {
        return Err("Zephyr Spirit Blocks-effect mutation escaped the live trace".to_owned());
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
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
        "card": {"collector_number": 76, "definition_id": CARD_ID, "name": "Zephyr Spirit"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "302.6: summoning sickness does not restrict blocking",
                "509.1: blocker declaration and the Blocks event",
                "603.2: the declared-blocker event triggers the ability",
                "117 and 608: the triggered ability uses and resolves from the stack",
                "400.7 and 701.21: source return to its owner's hand",
                "510.1 and 510.2: combat proceeds after the blocker leaves",
                "601.2: actual colored creature-spell costs",
            ],
            "source_contract": "RAV Zephyr Spirit identity, {5}{U} cost, 0/6 body, and mandatory Blocks trigger returning that source to its owner's hand",
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
        "qualification_status": "reference_trace_only; independent reviewer approval and provider trial pending",
        "mutation_sensitivity": {
            "blocks_effect_mutation_detected_in_game": mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    for seed in 0..64 {
        match run_match(seed, false) {
            Ok(captured) => match validate_trace(&captured, seed) {
                Ok(_) => return Ok((seed, captured)),
                Err(error)
                    if error.contains("was not drawn")
                        || error.contains("was not cast")
                        || error.contains("did not resolve") => {}
                Err(error) => return Err(error),
            },
            Err(error) if error.contains("not drawn") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a complete Zephyr Spirit Blocks trace".to_owned())
}

fn blocks_effect_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, true)?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("was not returned to its owner's hand after its Blocks trigger")
    }))
}

fn run_match(seed: u64, mutate_effect: bool) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WIN_CONDITION_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 17,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 17,
            },
        ],
        sideboard: vec![],
    };
    let attacker_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ATTACKER_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 59,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [(P0_DECK, &pilot_deck), (P1_DECK, &attacker_deck)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{label} deck has {size} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Zephyr Spirit definition is missing from the RAV registry")?;
    *registered = candidate_zephyr_spirit::definition();
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    let mut trigger = candidate_zephyr_spirit::triggered_ability();
    if mutate_effect {
        trigger.ability.effects = vec![Effect::DrawController];
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
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &attacker_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ZephyrPolicy::new(PlayerId(0))),
        Box::new(ElfPolicy::new(PlayerId(1))),
    ];
    let (mut attempted_moves, mut accepted_moves) = (0_u32, 0_u32);
    let mut zephyr = None;
    let mut attacker = None;
    let mut watcher = None;
    let mut zephyr_payment = Vec::new();
    let mut attacker_payment = Vec::new();
    let mut watcher_pt: Option<(i32, i32)> = None;
    let mut zephyr_pt: Option<(i32, i32)> = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Zephyr Spirit PvP match exceeded its development bound".to_owned());
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
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } => {
                    let definition = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?;
                    if definition.id == CARD_ID {
                        zephyr = Some(*card);
                        zephyr_payment = colors.clone();
                    } else if definition.id == WIN_CONDITION_ID {
                        watcher = Some(*card);
                    }
                    if definition.id == WIN_CONDITION_ID {
                        watcher = Some(*card);
                    }
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } => {
                    let definition = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?;
                    if definition.id == ATTACKER_ID {
                        attacker = Some(*card);
                        attacker_payment = colors.clone();
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } => {
                    let definition = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?;
                    if definition.id == CARD_ID {
                        zephyr = Some(*card);
                    }
                    if definition.id == WIN_CONDITION_ID && watcher.is_none() {
                        watcher = Some(*card);
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } => {
                    let definition = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?;
                    if definition.id == ATTACKER_ID && attacker.is_none() {
                        attacker = Some(*card);
                    }
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } => {
                    if Some(*card) == zephyr {
                        if let Ok(characteristics) = game.characteristics(*card) {
                            zephyr_pt = Some((
                                characteristics.power.unwrap_or(-1),
                                characteristics.toughness.unwrap_or(-1),
                            ));
                        }
                    }
                    if Some(*card) == watcher {
                        if let Ok(characteristics) = game.characteristics(*card) {
                            watcher_pt = Some((
                                characteristics.power.unwrap_or(-1),
                                characteristics.toughness.unwrap_or(-1),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let terminal = winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    let life_totals: [i64; 2] = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal,
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        zephyr,
        attacker,
        watcher,
        zephyr_payment,
        attacker_payment,
        watcher_pt,
        zephyr_pt,
        zephyr_zone: zephyr.and_then(|card| game.zone_of(card)),
        life_totals,
    })
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    zephyr: Option<ObjectId>,
    attacker: Option<ObjectId>,
    watcher: Option<ObjectId>,
    zephyr_payment: Vec<Color>,
    attacker_payment: Vec<Color>,
    watcher_pt: Option<(i32, i32)>,
    zephyr_pt: Option<(i32, i32)>,
    zephyr_zone: Option<Zone>,
    life_totals: [i64; 2],
}

struct ZephyrPolicy {
    player: PlayerId,
    zephyr_cast: bool,
    zephyr_blocked: bool,
    watcher_cast: bool,
}

impl ZephyrPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            zephyr_cast: false,
            zephyr_blocked: false,
            watcher_cast: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let count = |color| {
            view.own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
                })
                .count()
        };
        let needed = [Color::Green, Color::White, Color::Blue]
            .into_iter()
            .find(|color| count(*color) == 0);
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && needed.is_some_and(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn tap_for(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
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

    fn make_mana(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        for color in &cost.colored {
            let required = cost
                .colored
                .iter()
                .filter(|candidate| *candidate == color)
                .count() as u8;
            if view.mana_pool.amount(*color) < required {
                return self.tap_for(view, *color);
            }
        }
        let required = cost.generic.saturating_add(cost.colored.len() as u8);
        if view.mana_pool.total() < required {
            return view
                .own_battlefield
                .iter()
                .find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
                .and_then(|land| {
                    Some(PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: land.mana_colors.iter().next().copied()?,
                    })
                });
        }
        None
    }

    fn cast(&mut self, view: &GameView, definition: &str, cost: ManaCost) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = self.payment(view, &cost) {
            if definition == CARD_ID {
                self.zephyr_cast = true;
            }
            if definition == WIN_CONDITION_ID {
                self.watcher_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
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

impl CodePolicy for ZephyrPolicy {
    fn id(&self) -> &'static str {
        "rav.zephyr-spirit-block-trigger.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(WIN_CONDITION_ID) && card.can_attack)
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.zephyr_blocked {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(ATTACKER_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.zephyr_blocked = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![cardbench_magic_engine::CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
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
        if !self.watcher_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(WIN_CONDITION_ID))
        {
            if let Some(action) = self.cast(
                view,
                WIN_CONDITION_ID,
                ManaCost::with_colors(0, [Color::Green, Color::White]),
            ) {
                return action;
            }
        }
        if !self.zephyr_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(action) = self.cast(view, CARD_ID, ManaCost::with_colors(5, [Color::Blue]))
            {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

struct ElfPolicy {
    player: PlayerId,
    elf_cast: bool,
}

impl ElfPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_cast: false,
        }
    }
}

impl CodePolicy for ElfPolicy {
    fn id(&self) -> &'static str {
        "rav.deep-shadow-attack.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(ATTACKER_ID) && card.can_attack)
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
        if view.active_player != self.player
            || view.priority != self.player
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
        if !self.elf_cast {
            if let Some(elf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ATTACKER_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    self.elf_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: elf.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![],
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && card.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(
            "Zephyr Spirit full PvP game did not reach a clean terminal player-zero win".to_owned(),
        );
    }
    let zephyr = captured.zephyr.ok_or("Zephyr Spirit was not cast")?;
    let attacker = captured
        .attacker
        .ok_or("Elves of Deep Shadow was not cast")?;
    let watcher = captured.watcher.ok_or("Watchwolf was not cast")?;
    if captured.zephyr_zone != Some(Zone::Hand) {
        return Err(
            "Zephyr Spirit was not returned to its owner's hand after its Blocks trigger"
                .to_owned(),
        );
    }
    if captured.zephyr_pt != Some((0, 6)) || captured.watcher_pt != Some((3, 3)) {
        return Err(format!(
            "live cast characteristics differ: Zephyr={:?}, Watchwolf={:?}",
            captured.zephyr_pt, captured.watcher_pt
        ));
    }
    if captured.zephyr_payment.len() != 6 || !captured.zephyr_payment.contains(&Color::Blue) {
        return Err(format!(
            "Zephyr Spirit did not pay its full six-mana cost including blue: {:?}; receipts={:?}",
            captured.zephyr_payment,
            captured
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::SpellManaPaid { .. }))
                .collect::<Vec<_>>()
        ));
    }
    if captured.attacker_payment.len() != 1 || !captured.attacker_payment.contains(&Color::Green) {
        return Err(format!(
            "opposing attacker did not pay its green cost: {:?}",
            captured.attacker_payment
        ));
    }
    if !captured.events.iter().any(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == watcher && colors.contains(&Color::Green) && colors.contains(&Color::White))) {
        return Err("Watchwolf was not paid with actual green and white mana".to_owned());
    }
    let zephyr_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == zephyr)).ok_or("Zephyr Spirit cast receipt missing")?;
    let zephyr_resolved = event_index_after(
        &captured.events,
        zephyr_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == zephyr),
    )
    .ok_or("Zephyr Spirit did not resolve")?;
    let zephyr_entered = event_index_after(&captured.events, zephyr_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == zephyr)).ok_or("Zephyr Spirit battlefield entry receipt missing")?;
    let attacker_attack = event_index_after(&captured.events, zephyr_entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&attacker))).ok_or("the opponent's real ground creature did not attack after Zephyr Spirit entered")?;
    let blocker_assignment = event_index_after(&captured.events, attacker_attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.iter().any(|block| block.0 == attacker && block.1 == zephyr))).ok_or("Zephyr Spirit did not make the legal declared block")?;
    let trigger_stacked = event_index_after(&captured.events, blocker_assignment, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability, .. } if *source == zephyr && *ability == TRIGGER_ID)).ok_or("Zephyr Spirit Blocks trigger did not reach the stack")?;
    let returned = event_index_after(
        &captured.events,
        trigger_stacked,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == zephyr),
    )
    .ok_or("Zephyr Spirit was not returned to its owner's hand after its Blocks trigger")?;
    let trigger_resolved = event_index_after(&captured.events, trigger_stacked, |event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == zephyr && *ability == TRIGGER_ID)).ok_or("Zephyr Spirit Blocks trigger did not resolve")?;
    if returned >= trigger_resolved {
        return Err("Zephyr Spirit source moved before its Blocks trigger resolved".to_owned());
    }
    if captured.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == zephyr)) {
        return Err("Zephyr Spirit died in the blocking combat instead of returning to hand".to_owned());
    }
    let attacker_damage = event_index_after(&captured.events, returned, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount: 1 } if *source == attacker)).ok_or("combat damage did not proceed after the triggered blocker returned")?;
    let watcher_attack = event_index_after(&captured.events, attacker_damage, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watcher))).ok_or("Watchwolf did not attack after the source-relative return")?;
    let watcher_damage = event_index_after(&captured.events, watcher_attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watcher)).ok_or("Watchwolf did not deal three later-turn combat damage")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal player-zero game event missing")?;
    if !(zephyr_cast < zephyr_resolved
        && zephyr_resolved < zephyr_entered
        && zephyr_entered < attacker_attack
        && attacker_attack < blocker_assignment
        && blocker_assignment < trigger_stacked
        && trigger_stacked < returned
        && returned < trigger_resolved
        && trigger_resolved < attacker_damage
        && attacker_damage < watcher_attack
        && watcher_attack < watcher_damage
        && watcher_damage < ended)
    {
        return Err("Zephyr Spirit cast, Blocks trigger, return, resumed combat, and terminal event order differs".to_owned());
    }
    if captured.life_totals[1] > 0 {
        return Err(format!(
            "terminal player-one life total was {}, expected zero or less",
            captured.life_totals[1]
        ));
    }
    let projected = project_events(&captured.events);
    let relevant_kinds: Vec<&str> = projected.iter().map(|event| event.kind.as_str()).collect();
    let ordered = [
        zephyr_cast,
        zephyr_resolved,
        zephyr_entered,
        attacker_attack,
        blocker_assignment,
        trigger_stacked,
        returned,
        trigger_resolved,
        attacker_damage,
        watcher_attack,
        watcher_damage,
        ended,
    ];
    Ok(json!({
        "id": "rav-zephyr-spirit-blocks-and-returns-to-owner-hand",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.zephyr-spirit-block-trigger.v1", "rav.deep-shadow-attack.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "life_at_match_end": captured.life_totals,
        "zephyr_object": zephyr.0,
        "attacker_object": attacker.0,
        "watchwolf_object": watcher.0,
        "zephyr_payment": captured.zephyr_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "attacker_payment": captured.attacker_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "live_characteristics": {
            "zephyr_spirit": {"power": captured.zephyr_pt.unwrap().0, "toughness": captured.zephyr_pt.unwrap().1},
            "watchwolf": {"power": captured.watcher_pt.unwrap().0, "toughness": captured.watcher_pt.unwrap().1}
        },
        "zephyr_zone_at_terminal": format!("{:?}", captured.zephyr_zone),
        "ordered_event_sequence": ordered,
        "projected_event_kinds": relevant_kinds,
        "events": projected,
        "passed": true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(next, event)| predicate(event).then_some(next))
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_zephyr_spirit::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Zephyr Spirit is absent from the RAV definition registry")?;
    if definition != registered {
        return Err(
            "candidate Zephyr Spirit definition differs from its RAV registry binding".to_owned(),
        );
    }
    let trigger = candidate_zephyr_spirit::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Zephyr Spirit Blocks trigger is absent from the RAV trigger registry")?;
    if trigger != registered_trigger {
        return Err(
            "candidate Zephyr Spirit trigger differs from its RAV registry binding".to_owned(),
        );
    }
    if trigger.ability.condition != TriggerCondition::Blocks {
        return Err("Zephyr Spirit trigger condition is not Blocks".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Zephyr Spirit source: {error}"))?;
    if source.len() > 64 * 1024 {
        return Err("Zephyr Spirit source exceeds 64 KiB".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Zephyr Spirit source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Zephyr Spirit source has a non-documentation module attribute".to_owned());
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
                    return Err("Zephyr Spirit source contains an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) {
                    return Err("Zephyr Spirit source has duplicate functions".to_owned());
                }
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => {
                        return Err(
                            "Zephyr Spirit source contains an unexpected function".to_owned()
                        )
                    }
                };
                validate_candidate_function(function, expected_type)?;
                let Some(Stmt::Expr(Expr::Struct(structure), None)) = function.block.stmts.first()
                else {
                    return Err(
                        "Zephyr Spirit functions must return direct declarative literals"
                            .to_owned(),
                    );
                };
                if function.block.stmts.len() != 1
                    || !safe_candidate_expression(&Expr::Struct(structure.clone()))
                {
                    return Err("Zephyr Spirit functions may contain only safe static card and binding literals".to_owned());
                }
            }
            _ => return Err("Zephyr Spirit source contains executable top-level code".to_owned()),
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Zephyr Spirit imports or module structure differ from contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_function(function: &syn::ItemFn, return_type: &str) -> Result<(), String> {
    let visibility_ok =
        matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
    let output_ok = match &function.sig.output {
        syn::ReturnType::Type(_, ty) => match &**ty {
            Type::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == return_type),
            _ => false,
        },
        syn::ReturnType::Default => false,
    };
    if !visibility_ok
        || !output_ok
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.constness.is_some()
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
        || !function.attrs.iter().all(is_safe_function_attribute)
    {
        return Err(format!(
            "Zephyr Spirit function must be a plain pub(crate) {return_type} factory"
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
        Expr::Tuple(tuple) => {
            tuple.attrs.is_empty() && tuple.elems.iter().all(safe_candidate_expression)
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
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let name = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            matches!(
                name.as_str(),
                "BTreeSet::from"
                    | "BTreeSet::new"
                    | "ManaCost::with_colors"
                    | "ManaCost::new"
                    | "Some"
            ) && call.args.iter().all(safe_candidate_expression)
        }
        Expr::Struct(structure) => {
            if !structure.attrs.is_empty() || structure.rest.is_some() {
                return false;
            }
            let name = structure
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            matches!(
                name.as_deref(),
                Some("CardDefinition" | "TriggeredAbilityBinding" | "TriggeredAbility")
            ) && structure
                .fields
                .iter()
                .all(|field| field.attrs.is_empty() && safe_candidate_expression(&field.expr))
        }
        Expr::Macro(mac) => {
            mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => definition_id,
        _ => "",
    };
    let passed = definition_id == CARD_ID;
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":"executable_compatibility_slice","qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--output-root" {
            return arguments.next().map(Into::into);
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
