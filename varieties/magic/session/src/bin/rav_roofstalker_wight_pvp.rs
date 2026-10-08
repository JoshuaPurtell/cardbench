//! Full-game RAV Roofstalker Wight authority for colored casting and temporary Flying.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, Color, CombatBlock, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, Layer, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ROOFSTALKER-WIGHT";
const OPPONENT_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/roofstalker_wight.rs";
const P0_DECK: &str = "rav_roofstalker_wight_self_flying";
const P1_DECK: &str = "rav_watchwolf_ground_defense";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/roofstalker_wight.rs"]
mod candidate_roofstalker_wight;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Roofstalker Wight PvP result: {error}");
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
            eprintln!("Roofstalker Wight PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = 0;
    let definition = candidate_roofstalker_wight::definition();
    let ability = candidate_roofstalker_wight::activated_ability();
    let captured = run_match(definition.clone(), ability.clone(), seed, true)?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(definition.clone(), ability.clone(), seed, true)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.wight != replay.wight
        || captured.watchwolf != replay.watchwolf
        || captured.first_cleanup_expired_flying != replay.first_cleanup_expired_flying
        || captured.first_post_cleanup_no_flying != replay.first_post_cleanup_no_flying
    {
        return Err("Roofstalker Wight full-game replay was not deterministic".to_owned());
    }
    let mutation_detected = flying_removal_mutation_fails(seed)?;
    if !mutation_detected {
        return Err(
            "removing Roofstalker Wight's self-Flying effect escaped the combat trace".to_owned(),
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
        "card": {"collector_number": 102, "definition_id": CARD_ID, "name": "Roofstalker Wight"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: black identity and colored payment",
                "106.1 and 106.4: mana production and payment",
                "117: priority and activated abilities",
                "302.1-302.6: creature permanents, combat, and summoning sickness",
                "508: declaring attackers",
                "509.1: declaring blockers and Flying legality",
                "601.2 and 608.3a: casting and resolving creature spells",
                "602.1-602.2: activated-ability costs and stack resolution",
                "611.2a and 613: until-end-of-turn continuous effects",
                "702.9: Flying"
            ],
            "source_contract": "RAV Roofstalker Wight identity, black cast cost, and candidate-owned self-Flying activation",
            "authority_cutoff": "2026-09-23"
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
            "self_flying_removed_mutation_detected": mutation_detected,
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
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
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
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    wight: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    block_attempt_seen: bool,
    block_attempt_rejected: bool,
    block_attempt_accepted: bool,
    attempt_event_count_before: Option<usize>,
    attempt_event_count_after: Option<usize>,
    block_rejection_reason: Option<String>,
    blocker_live_at_attempt: bool,
    blocker_has_flying_or_reach: bool,
    wight_flying_at_attempt: bool,
    wight_flying_at_attack: bool,
    first_cleanup_expired_flying: bool,
    first_post_cleanup_no_flying: bool,
    wight_cast_payment: Vec<Color>,
    watchwolf_cast_payment: Vec<Color>,
    first_activation_mana_colors: Vec<Color>,
    wight_live: bool,
    live_flying: bool,
    live_power_toughness: Option<(i32, i32)>,
}

fn run_match(
    definition: CardDefinition,
    ability: ActivatedAbility,
    seed: u64,
    stop_after_accepted_block: bool,
) -> Result<Captured, String> {
    let wight_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let watchwolf_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_ID.to_owned(),
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
    for (name, deck) in [(P0_DECK, &wight_deck), (P1_DECK, &watchwolf_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition.clone(), ability)?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &wight_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &watchwolf_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let watchwolf_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == OPPONENT_ID)
        .ok_or("Watchwolf definition missing")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(WightPolicy::new(PlayerId(0), definition.mana_cost)),
        Box::new(WightPolicy::new(PlayerId(1), watchwolf_cost)),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut block_attempt_seen = false;
    let mut block_attempt_rejected = false;
    let mut block_attempt_accepted = false;
    let mut attempt_event_count_before = None;
    let mut attempt_event_count_after = None;
    let mut block_rejection_reason = None;
    let mut blocker_live_at_attempt = false;
    let mut blocker_has_flying_or_reach = false;
    let mut wight_flying_at_attempt = false;
    let mut wight_flying_at_attack = false;
    let mut first_cleanup_expired_flying = false;
    let mut first_post_cleanup_no_flying = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Roofstalker Wight match exceeded its development bound".to_owned());
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
        let challenge = player == PlayerId(1)
            && matches!(&action, PolicyAction::DeclareBlockers { assignments } if assignments.len() == 1);
        if challenge {
            block_attempt_seen = true;
            attempt_event_count_before = Some(game.event_log.len());
            if let PolicyAction::DeclareBlockers { assignments } = &action {
                if let Some(block) = assignments.first() {
                    blocker_live_at_attempt =
                        game.zone_of(block.blocker) == Some(Zone::Battlefield);
                    blocker_has_flying_or_reach =
                        game.characteristics(block.blocker).is_ok_and(|c| {
                            c.keywords.contains(&Keyword::Flying)
                                || c.keywords.contains(&Keyword::Reach)
                        });
                    wight_flying_at_attempt = game
                        .characteristics(block.attacker)
                        .is_ok_and(|c| c.keywords.contains(&Keyword::Flying));
                }
            }
        }
        if player == PlayerId(0) {
            if let PolicyAction::DeclareAttackers { attackers } = &action {
                wight_flying_at_attack = attackers
                    .iter()
                    .filter(|object| {
                        game.card_definition(**object)
                            .is_ok_and(|card| card.id == CARD_ID)
                    })
                    .any(|object| {
                        game.characteristics(*object)
                            .is_ok_and(|c| c.keywords.contains(&Keyword::Flying))
                    });
            }
        }
        attempted += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => {
                accepted += 1;
                if challenge {
                    block_attempt_accepted = true;
                    attempt_event_count_after = Some(game.event_log.len());
                }
            }
            Err(error) if challenge => {
                block_attempt_rejected = true;
                block_rejection_reason = Some(error.to_string());
                attempt_event_count_after = Some(game.event_log.len());
                if attempt_event_count_before != attempt_event_count_after {
                    return Err(
                        "Roofstalker Wight rejected block attempt changed the event log".to_owned(),
                    );
                }
            }
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        if !first_cleanup_expired_flying && game.event_log.iter().any(|event| matches!(event, GameEvent::ContinuousEffectExpired { source, target, layer: Layer::Ability } if *source == *target && game.card_definition(*source).is_ok_and(|card| card.id == CARD_ID))) {
            first_cleanup_expired_flying = true;
            let wight_object = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast { player: PlayerId(0), card } if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID) => Some(*card),
                _ => None,
            });
            first_post_cleanup_no_flying = wight_object.is_some_and(|object| game.characteristics(object).is_ok_and(|c| !c.keywords.contains(&Keyword::Flying)));
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if challenge && block_attempt_accepted && stop_after_accepted_block {
            break;
        }
    }
    let events = game.event_log.clone();
    let wight = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|d| d.id == OPPONENT_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let wight_cast_payment = spell_payment(&events, PlayerId(0), wight);
    let watchwolf_cast_payment = spell_payment(&events, PlayerId(1), watchwolf);
    let first_activation = event_index(
        &events,
        |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", mana_cost } if Some(*source) == wight && *mana_cost == ManaCost::with_colors(1, [Color::Blue])),
    );
    let first_activation_mana_colors = first_activation
        .map(|index| {
            let start = events[..index]
                .iter()
                .rposition(|event| matches!(event, GameEvent::StepBegan { .. }))
                .unwrap_or(0);
            events[start..index]
                .iter()
                .filter_map(|event| match event {
                    GameEvent::ManaAbilityActivated {
                        player: PlayerId(0),
                        color,
                        ..
                    } => Some(*color),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let characteristics = wight.and_then(|object| game.characteristics(object).ok());
    let live_flying = characteristics
        .as_ref()
        .is_some_and(|c| c.keywords.contains(&Keyword::Flying));
    let live_power_toughness = characteristics.and_then(|c| Some((c.power?, c.toughness?)));
    let wight_live = wight.is_some_and(|object| game.zone_of(object) == Some(Zone::Battlefield));
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: if game.is_game_over() {
            winner.map_or_else(
                || "Draw".to_owned(),
                |seat| format!("Winner(PlayerId({seat}))"),
            )
        } else {
            "InProgress".to_owned()
        },
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        wight,
        watchwolf,
        block_attempt_seen,
        block_attempt_rejected,
        block_attempt_accepted,
        attempt_event_count_before,
        attempt_event_count_after,
        block_rejection_reason,
        blocker_live_at_attempt,
        blocker_has_flying_or_reach,
        wight_flying_at_attempt,
        wight_flying_at_attack,
        first_cleanup_expired_flying,
        first_post_cleanup_no_flying,
        wight_cast_payment,
        watchwolf_cast_payment,
        first_activation_mana_colors,
        wight_live,
        live_flying,
        live_power_toughness,
    })
}

fn spell_payment(events: &[GameEvent], player: PlayerId, card: Option<ObjectId>) -> Vec<Color> {
    card.and_then(|card| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: paid_player,
                card: paid_card,
                colors,
            } if *paid_player == player && *paid_card == card => Some(colors.clone()),
            _ => None,
        })
    })
    .unwrap_or_default()
}

fn new_game(definition: CardDefinition, ability: ActivatedAbility) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Roofstalker Wight definition is missing")?;
    *registered = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| {
            binding.card_definition == CARD_ID && binding.ability.id == "blue-gain-flying"
        })
        .ok_or("Roofstalker Wight binding is missing")?;
    binding.ability = ability;
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())
}

struct WightPolicy {
    player: PlayerId,
    cost: ManaCost,
    wight_player: bool,
    cast_once: bool,
    challenged: bool,
    activation_turn: Option<u32>,
}

impl WightPolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            cost,
            wight_player: player == PlayerId(0),
            cast_once: false,
            challenged: false,
            activation_turn: None,
        }
    }
    fn choose_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let colors = if self.wight_player {
            [Color::Blue, Color::Black]
        } else {
            [Color::White, Color::Green]
        };
        let count = |color| {
            view.own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
                })
                .count()
        };
        let preferred = if count(colors[0]) <= count(colors[1]) {
            colors[0]
        } else {
            colors[1]
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&preferred)
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
    fn mana_action(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        cost.colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .and_then(|color| self.tap_for(view, *color))
            .or_else(|| {
                Color::MANA_ALL
                    .into_iter()
                    .find(|color| self.tap_for(view, *color).is_some())
                    .and_then(|color| self.tap_for(view, color))
            })
    }
    fn cast_action(
        &mut self,
        view: &GameView,
        id: &'static str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        if let Some(mana_selection) = self.payment(view, cost) {
            self.cast_once = true;
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
        self.mana_action(view, cost)
    }
}

impl CodePolicy for WightPolicy {
    fn id(&self) -> &'static str {
        if self.wight_player {
            "rav.roofstalker-wight-self-flying.v1"
        } else {
            "rav.watchwolf-ground-blocker.v1"
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
            let attackers = if self.wight_player {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if !self.wight_player
            && view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.challenged {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(OPPONENT_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.challenged = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
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
        if let Some(action) = self.choose_land(view) {
            return action;
        }
        if self.wight_player {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            if let Some(source) =
                source.filter(|card| card.can_attack && self.activation_turn != Some(view.turn))
            {
                let cost = ManaCost::with_colors(1, [Color::Blue]);
                if self.payment(view, &cost).is_some() {
                    self.activation_turn = Some(view.turn);
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: "blue-gain-flying",
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
                if let Some(action) = self.mana_action(view, &cost) {
                    return action;
                }
            }
            if !self.cast_once {
                if let Some(action) = self.cast_action(view, CARD_ID, &self.cost.clone()) {
                    return action;
                }
            }
        } else if !self.cast_once {
            if let Some(action) = self.cast_action(view, OPPONENT_ID, &self.cost.clone()) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let wight = captured.wight.ok_or("Roofstalker Wight was not cast")?;
    let watchwolf = captured
        .watchwolf
        .ok_or("opposing Watchwolf was not cast")?;
    if !captured.block_attempt_seen
        || !captured.block_attempt_rejected
        || captured.block_attempt_accepted
        || !captured.wight_flying_at_attempt
        || captured.blocker_has_flying_or_reach
    {
        return Err(
            "Roofstalker Wight must gain Flying before Watchwolf's block attempt".to_owned(),
        );
    }
    if captured.rejected_moves != 1
        || captured.attempt_event_count_before != captured.attempt_event_count_after
    {
        return Err("Roofstalker Wight flying block rejection was not event-atomic".to_owned());
    }
    if !captured.blocker_live_at_attempt {
        return Err("the challenged Watchwolf must be a live blocker".to_owned());
    }
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Roofstalker Wight PvP game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if !captured.wight_live
        || !captured.live_flying
        || captured.live_power_toughness != Some((2, 1))
        || !captured.wight_flying_at_attack
    {
        return Err("Roofstalker Wight must remain a live 2/1 and attack with Flying".to_owned());
    }
    if !captured.first_cleanup_expired_flying || !captured.first_post_cleanup_no_flying {
        return Err("Roofstalker Wight Flying did not expire at cleanup before the next turn's reactivation".to_owned());
    }
    let black_wight = captured
        .wight_cast_payment
        .iter()
        .filter(|color| **color == Color::Black)
        .count();
    if black_wight < 1 || captured.wight_cast_payment.len() != 2 {
        return Err(format!(
            "Roofstalker Wight cast paid {:?}; black plus one generic mana is required",
            captured.wight_cast_payment
        ));
    }
    let white_watchwolf = captured
        .watchwolf_cast_payment
        .iter()
        .filter(|color| **color == Color::White)
        .count();
    let green_watchwolf = captured
        .watchwolf_cast_payment
        .iter()
        .filter(|color| **color == Color::Green)
        .count();
    if white_watchwolf < 1 || green_watchwolf < 1 || captured.watchwolf_cast_payment.len() != 2 {
        return Err(format!(
            "opposing Watchwolf cast paid {:?}; green and white mana are required",
            captured.watchwolf_cast_payment
        ));
    }
    if captured.first_activation_mana_colors.len() != 2
        || !captured.first_activation_mana_colors.contains(&Color::Blue)
    {
        return Err(format!(
            "Roofstalker Wight `{{1}}{{U}}` activation was not paid with blue plus generic sources: {:?}",
            captured.first_activation_mana_colors
        ));
    }
    let wight_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == wight && colors.len() == 2 && colors.contains(&Color::Black))).ok_or("Roofstalker Wight SpellManaPaid receipt missing")?;
    let wight_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wight)).ok_or("Roofstalker Wight SpellCast receipt missing")?;
    let wight_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == wight),
    )
    .ok_or("Roofstalker Wight SpellResolved receipt missing")?;
    let watchwolf_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == watchwolf && colors.len() == 2 && colors.contains(&Color::White) && colors.contains(&Color::Green))).ok_or("Watchwolf green/white SpellManaPaid receipt missing")?;
    let watchwolf_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == watchwolf)).ok_or("Watchwolf SpellCast receipt missing")?;
    let watchwolf_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf SpellResolved receipt missing")?;
    let first_paid = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", mana_cost } if *source == wight && *mana_cost == ManaCost::with_colors(1, [Color::Blue]))).ok_or("Roofstalker Wight AbilityManaPaid receipt missing")?;
    let first_activated = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition, ability: "blue-gain-flying", .. } if *source == wight && *definition == CARD_ID)).ok_or("Roofstalker Wight AbilityActivated receipt missing")?;
    let first_effect = event_index(&captured.events, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, layer: Layer::Ability } if *source == wight && *target == wight)).ok_or("Roofstalker Wight self-Flying continuous effect receipt missing")?;
    let first_resolved = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: "blue-gain-flying", .. } if *source == wight)).ok_or("Roofstalker Wight AbilityResolved receipt missing")?;
    let attack = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wight))).ok_or("Roofstalker Wight did not attack")?;
    let empty_blockers = event_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("legal empty blocker declaration after Flying block rejection missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == wight)).ok_or("Roofstalker Wight combat damage missing")?;
    let expiry = event_index(&captured.events, |event| matches!(event, GameEvent::ContinuousEffectExpired { source, target, layer: Layer::Ability } if *source == wight && *target == wight)).ok_or("Roofstalker Wight cleanup expiry receipt missing")?;
    let second_paid = nth_ability_event(&captured.events, wight, 1, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight)).ok_or("Roofstalker Wight second activation after cleanup missing")?;
    let second_activated = nth_ability_event(&captured.events, wight, 1, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight)).ok_or("Roofstalker Wight second AbilityActivated receipt missing")?;
    let second_effect = nth_ability_event(&captured.events, wight, 1, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, layer: Layer::Ability } if *source == wight && *target == wight)).ok_or("Roofstalker Wight second Flying grant receipt missing")?;
    let second_resolved = nth_ability_event(&captured.events, wight, 1, |event| matches!(event, GameEvent::AbilityResolved { source, ability: "blue-gain-flying", .. } if *source == wight)).ok_or("Roofstalker Wight second AbilityResolved receipt missing")?;
    let end = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded receipt missing")?;
    if !(wight_paid < wight_cast
        && wight_cast < wight_resolved
        && watchwolf_paid < watchwolf_cast
        && watchwolf_cast < watchwolf_resolved
        && wight_resolved < first_paid
        && first_paid < first_activated
        && first_activated < first_effect
        && first_effect < first_resolved
        && first_resolved < attack
        && attack < empty_blockers
        && empty_blockers < damage
        && damage < expiry
        && expiry < second_paid
        && second_paid < second_activated
        && second_activated < second_effect
        && second_effect < second_resolved
        && second_resolved < end)
    {
        return Err("Roofstalker Wight cast, activated Flying, blocked combat, cleanup expiry, reactivation, and terminal event order differs".to_owned());
    }
    let events = relevant_events(&captured.events, wight, watchwolf)?;
    let kinds: Vec<_> = events.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "ManaAbilityActivated",
        "ManaAbilityActivated",
        "AbilityManaPaid",
        "AbilityActivated",
        "ContinuousEffectCreated",
        "AbilityResolved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "ContinuousEffectExpired",
        "AbilityManaPaid",
        "AbilityActivated",
        "ContinuousEffectCreated",
        "AbilityResolved",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Roofstalker Wight ordered projected events differ: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-roofstalker-wight-self-flying-and-cleanup-expiry",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.roofstalker-wight-self-flying.v1", "rav.watchwolf-ground-blocker.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "roofstalker_wight_object": wight.0,
        "watchwolf_object": watchwolf.0,
        "wight_cast_payment": {"required": {"black_symbols": 1, "generic_symbols": 1, "total_symbols": 2}, "spent": {"black": black_wight, "total": captured.wight_cast_payment.len()}},
        "watchwolf_cast_payment": {"required": {"green_symbols": 1, "white_symbols": 1, "total_symbols": 2}, "spent": {"green": green_watchwolf, "white": white_watchwolf, "total": captured.watchwolf_cast_payment.len()}},
        "activation_payment": {"required": {"blue_symbols": 1, "generic_symbols": 1, "total_symbols": 2}, "mana_abilities": captured.first_activation_mana_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "live_flying_at_attack": captured.wight_flying_at_attack,
        "live_flying_at_terminal": captured.live_flying,
        "resolved_power_toughness": {"power": 2, "toughness": 1},
        "cleanup_expiry": {"observed": captured.first_cleanup_expired_flying, "no_flying_after_cleanup_before_reactivation": captured.first_post_cleanup_no_flying},
        "block_attempt": {"defender_controls_live_ground_creature": captured.blocker_live_at_attempt && !captured.blocker_has_flying_or_reach, "wight_had_flying": captured.wight_flying_at_attempt, "rejected": captured.block_attempt_rejected, "engine_reason": captured.block_rejection_reason, "event_count_before": captured.attempt_event_count_before, "event_count_after": captured.attempt_event_count_after},
        "events": events,
        "passed": true
    }))
}

fn relevant_events(
    events: &[GameEvent],
    wight: ObjectId,
    watchwolf: ObjectId,
) -> Result<Vec<ProjectedEvent>, String> {
    let mut selected = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if matches!(event, GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } if *card == wight || *card == watchwolf)
            || matches!(event, GameEvent::SpellResolved { card } if *card == wight || *card == watchwolf)
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == wight || *card == watchwolf)
        {
            selected.push(index);
        }
    }
    let first_paid = event_index(events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight)).ok_or("first activation payment missing for projection")?;
    let turn_start = events[..first_paid]
        .iter()
        .rposition(|event| matches!(event, GameEvent::StepBegan { .. }))
        .unwrap_or(0);
    for (index, event) in events.iter().enumerate().take(first_paid).skip(turn_start) {
        if matches!(
            event,
            GameEvent::ManaAbilityActivated {
                player: PlayerId(0),
                ..
            }
        ) {
            selected.push(index);
        }
    }
    for matcher in 0..8 {
        let index = match matcher {
            0 => nth_ability_event(
                events,
                wight,
                0,
                |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            1 => nth_ability_event(
                events,
                wight,
                0,
                |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            2 => nth_ability_event(
                events,
                wight,
                0,
                |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, layer: Layer::Ability } if *source == wight && *target == wight),
            ),
            3 => nth_ability_event(
                events,
                wight,
                0,
                |event| matches!(event, GameEvent::AbilityResolved { source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            4 => nth_ability_event(
                events,
                wight,
                1,
                |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            5 => nth_ability_event(
                events,
                wight,
                1,
                |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            6 => nth_ability_event(
                events,
                wight,
                1,
                |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, layer: Layer::Ability } if *source == wight && *target == wight),
            ),
            7 => nth_ability_event(
                events,
                wight,
                1,
                |event| matches!(event, GameEvent::AbilityResolved { source, ability: "blue-gain-flying", .. } if *source == wight),
            ),
            _ => None,
        };
        if let Some(index) = index {
            selected.push(index);
        }
    }
    let (mut attack_selected, mut blockers_selected, mut damage_selected, mut expiry_selected) =
        (false, false, false, false);
    for (index, event) in events.iter().enumerate() {
        let include = match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if attackers.contains(&wight) && !attack_selected => {
                attack_selected = true;
                true
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } if assignments.is_empty() && !blockers_selected => {
                blockers_selected = true;
                true
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount: 2,
            } if *source == wight && !damage_selected => {
                damage_selected = true;
                true
            }
            GameEvent::ContinuousEffectExpired {
                source,
                target,
                layer: Layer::Ability,
            } if *source == wight && *target == wight && !expiry_selected => {
                expiry_selected = true;
                true
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        };
        if include {
            selected.push(index);
        }
    }
    selected.sort_unstable();
    selected.dedup();
    let chosen: BTreeSet<usize> = selected.into_iter().collect();
    let projected: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| chosen.contains(&index).then_some(event))
        .collect();
    if projected.len() != 23 {
        return Err(format!(
            "Roofstalker Wight event projection has {} receipts; expected 23",
            projected.len()
        ));
    }
    Ok(projected)
}

fn nth_ability_event(
    events: &[GameEvent],
    wight: ObjectId,
    ordinal: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let _ = wight;
    events
        .iter()
        .enumerate()
        .filter(|(_, event)| predicate(event))
        .nth(ordinal)
        .map(|(index, _)| index)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_digest(events: &[GameEvent]) -> String {
    let data = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let hash = data.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}

fn flying_removal_mutation_fails(seed: u64) -> Result<bool, String> {
    let mut ability = candidate_roofstalker_wight::activated_ability();
    ability.effects.clear();
    let captured = run_match(
        candidate_roofstalker_wight::definition(),
        ability,
        seed,
        true,
    )?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("Roofstalker Wight must gain Flying before Watchwolf's block attempt")
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Roofstalker Wight source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Roofstalker Wight source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Roofstalker Wight module attributes differ".to_owned());
    }
    let mut definition = None;
    let mut ability = None;
    let mut imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports.extend(
                    flatten_use_tree(&item.tree, Vec::new())
                        .ok_or("unsupported import in Roofstalker Wight source")?,
                );
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item)
            }
            Item::Fn(item) if item.sig.ident == "activated_ability" && ability.is_none() => {
                ability = Some(item)
            }
            _ => return Err("Roofstalker Wight source has executable top-level code".to_owned()),
        }
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if file.items.len() != 4
        || definition.is_none()
        || ability.is_none()
        || imports != expected_imports
    {
        return Err(
            "Roofstalker Wight source must contain only its imports and two literal definitions"
                .to_owned(),
        );
    }
    validate_function(definition.unwrap(), "definition", "CardDefinition", 13)?;
    validate_function(
        ability.unwrap(),
        "activated_ability",
        "ActivatedAbility",
        11,
    )?;
    let card = literal_fields_from_function(
        file.items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.sig.ident == "definition" => Some(function),
                _ => None,
            })
            .ok_or("Roofstalker Wight definition function missing")?,
        "CardDefinition",
    )?;
    check_string(card["id"], CARD_ID)?;
    check_string(card["name"], "Roofstalker Wight")?;
    check_string(card["set_code"], "RAV")?;
    check_mana_cost(card["mana_cost"], 1, &[Color::Black])?;
    check_set_paths(card["colors"], &[&["Color", "Black"]])?;
    check_empty_set(card["mana_colors"])?;
    check_set_paths(card["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(card["is_basic_land"], false)?;
    check_rules(card["supported_rules"])?;
    check_some_int(card["power"], 2)?;
    check_some_int(card["toughness"], 1)?;
    check_macro_tokens(card["keywords"], &[""], &[""])
        .map_err(|error| format!("Roofstalker Wight keywords: {error}"))?;
    check_empty_macro(card["effects"])?;
    let ability_fields = literal_fields_from_function(
        file.items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.sig.ident == "activated_ability" => Some(function),
                _ => None,
            })
            .ok_or("Roofstalker Wight activated ability function missing")?,
        "ActivatedAbility",
    )?;
    check_string(ability_fields["id"], "blue-gain-flying")?;
    check_mana_cost(ability_fields["mana_cost"], 1, &[Color::Blue])?;
    check_bool(ability_fields["tap_cost"], false)?;
    check_bool(ability_fields["sorcery_speed"], false)?;
    check_int(ability_fields["additional_tap_creatures"], 0)?;
    check_bool(ability_fields["sacrifice_source"], false)?;
    check_int(ability_fields["sacrifice_creatures"], 0)?;
    check_int(ability_fields["sacrifice_lands"], 0)?;
    check_int(ability_fields["discard_cards"], 0)?;
    check_macro_tokens(ability_fields["targets"], &[""], &[""])
        .map_err(|error| format!("Roofstalker Wight ability targets: {error}"))?;
    check_macro_tokens(
        ability_fields["effects"],
        &[
            "Effect::AddSourceKeywordUntilEndOfTurn{keyword:Keyword::Flying}",
            "Effect::AddSourceKeywordUntilEndOfTurn{keyword:Keyword::Flying,}",
            "Effect::AddSourceKeywordUntilEndOfTurn{keyword:Keyword::Reach}",
            "Effect::AddSourceKeywordUntilEndOfTurn{keyword:Keyword::Reach,}",
        ],
        &[""],
    )
    .map_err(|error| format!("Roofstalker Wight ability effects: {error}"))?;
    Ok(())
}

fn validate_function(
    function: &syn::ItemFn,
    name: &str,
    ty: &str,
    field_count: usize,
) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if !attrs_ok
        || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
        || function.sig.ident != name
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.block.stmts.len() != 1
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, output) if type_is(output, ty))
    {
        return Err(format!("Roofstalker Wight {name} signature differs"));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Roofstalker Wight {name} must return one direct literal"
        ));
    };
    if !path_is(&value.path, &[ty])
        || value.rest.is_some()
        || literal_fields(value)?.len() != field_count
    {
        return Err(format!("Roofstalker Wight {name} literal fields differ"));
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_roofstalker_wight::definition();
    let registry = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Roofstalker Wight is missing from the RAV registry")?;
    if candidate.id != registry.id
        || candidate.name != "Roofstalker Wight"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(1, [Color::Black])
        || candidate.colors != BTreeSet::from([Color::Black])
        || candidate.card_types != BTreeSet::from([CardType::Creature])
        || candidate.power != Some(2)
        || candidate.toughness != Some(1)
        || !candidate.keywords.is_empty()
        || !candidate.effects.is_empty()
    {
        return Err("Roofstalker Wight printed identity differs".to_owned());
    }
    let ability = candidate_roofstalker_wight::activated_ability();
    if ability.id != "blue-gain-flying"
        || ability.mana_cost != ManaCost::with_colors(1, [Color::Blue])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || !(ability.effects.is_empty()
            || ability.effects
                == [Effect::AddSourceKeywordUntilEndOfTurn {
                    keyword: Keyword::Flying,
                }]
            || ability.effects
                == [Effect::AddSourceKeywordUntilEndOfTurn {
                    keyword: Keyword::Reach,
                }])
    {
        return Err("Roofstalker Wight activated binding differs".to_owned());
    }
    Ok(())
}

fn literal_fields_from_function<'a>(
    function: &'a syn::ItemFn,
    ty: &str,
) -> Result<BTreeMap<String, &'a Expr>, String> {
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("candidate struct literal missing".to_owned());
    };
    if !path_is(&value.path, &[ty]) {
        return Err("candidate literal type differs".to_owned());
    }
    literal_fields(value)
}
fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("tuple field not accepted".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("shorthand or duplicate field not accepted".to_owned());
        }
    }
    Ok(fields)
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(&segment.arguments, PathArguments::None)
        })
}
fn type_is(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[expected]))
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Str(value) if value.value() == expected))
    {
        Ok(())
    } else {
        Err(format!("literal string must be {expected}"))
    }
}
fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Bool(value) if value.value == expected))
    {
        Ok(())
    } else {
        Err("boolean literal differs".to_owned())
    }
}
fn check_int(expr: &Expr, expected: u8) -> Result<(), String> {
    if matches!(expr, Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(value) if value.base10_parse::<u8>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("integer literal differs".to_owned())
    }
}
fn check_some_int(expr: &Expr, expected: i16) -> Result<(), String> {
    let args = call_args(expr, &["Some"]).ok_or("expected Some integer")?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(value) if value.base10_parse::<i16>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("Some integer differs".to_owned())
    }
}
fn call_args<'a>(expr: &'a Expr, path: &[&str]) -> Option<Vec<&'a Expr>> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let Expr::Path(function) = &*call.func else {
        return None;
    };
    if !path_is(&function.path, path) {
        return None;
    }
    Some(call.args.iter().collect())
}
fn check_mana_cost(expr: &Expr, generic: u8, colors: &[Color]) -> Result<(), String> {
    let args =
        call_args(expr, &["ManaCost", "with_colors"]).ok_or("expected ManaCost::with_colors")?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(lit) if matches!(&lit.lit, Lit::Int(value) if value.base10_parse::<u8>().ok() == Some(generic)))
    {
        return Err("mana generic cost differs".to_owned());
    }
    let Expr::Array(array) = args[1] else {
        return Err("mana colors must be an array".to_owned());
    };
    if array.elems.len() != colors.len() {
        return Err("mana color count differs".to_owned());
    }
    for (expr, color) in array.elems.iter().zip(colors) {
        let path = match color {
            Color::Black => ["Color", "Black"],
            Color::Blue => ["Color", "Blue"],
            Color::Green => ["Color", "Green"],
            Color::White => ["Color", "White"],
            _ => return Err("unsupported expected color".to_owned()),
        };
        let Expr::Path(value) = expr else {
            return Err("mana color must be literal".to_owned());
        };
        if !path_is(&value.path, &path) {
            return Err("mana color differs".to_owned());
        }
    }
    Ok(())
}
fn check_set_paths(expr: &Expr, expected: &[&[&str]]) -> Result<(), String> {
    let args = call_args(expr, &["BTreeSet", "from"]).ok_or("expected BTreeSet::from")?;
    if args.len() != 1 {
        return Err("set arg count differs".to_owned());
    }
    let Expr::Array(array) = args[0] else {
        return Err("set values must be an array".to_owned());
    };
    if array.elems.len() != expected.len() {
        return Err("set value count differs".to_owned());
    }
    for (expr, path) in array.elems.iter().zip(expected) {
        let Expr::Path(value) = expr else {
            return Err("set entry must be an enum path".to_owned());
        };
        if !path_is(&value.path, path) {
            return Err("set entry differs".to_owned());
        }
    }
    Ok(())
}
fn check_empty_set(expr: &Expr) -> Result<(), String> {
    let args = call_args(expr, &["BTreeSet", "new"]).ok_or("expected BTreeSet::new")?;
    if args.is_empty() {
        Ok(())
    } else {
        Err("expected empty set".to_owned())
    }
}
fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("supported rules must be a reference".to_owned());
    };
    let Expr::Array(array) = &*reference.expr else {
        return Err("supported rules must be a literal array".to_owned());
    };
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "self-flying-until-end-of-turn",
    ];
    if array.elems.len() != expected.len() {
        return Err("supported rules count differs".to_owned());
    }
    for (expr, rule) in array.elems.iter().zip(expected) {
        check_string(expr, rule)?;
    }
    Ok(())
}
fn check_macro_tokens(expr: &Expr, allowed: &[&str], empty_allowed: &[&str]) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("expected vec! macro".to_owned());
    };
    if !mac.mac.path.is_ident("vec") {
        return Err("expected vec! macro".to_owned());
    }
    let normalized = mac
        .mac
        .tokens
        .to_string()
        .replace(' ', "")
        .trim_end_matches(',')
        .to_owned();
    if allowed.iter().any(|value| *value == normalized)
        || normalized.is_empty() && !empty_allowed.is_empty()
    {
        Ok(())
    } else {
        Err("literal vector content differs".to_owned())
    }
}
fn check_empty_macro(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("expected empty vec! macro".to_owned());
    };
    if mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty() {
        Ok(())
    } else {
        Err("expected empty vec! macro".to_owned())
    }
}
fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>) -> Option<Vec<Vec<String>>> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            flatten_use_tree(&path.tree, next)
        }
        UseTree::Name(name) => {
            let mut next = prefix;
            next.push(name.ident.to_string());
            Some(vec![next])
        }
        UseTree::Group(group) => {
            let mut all = Vec::new();
            for item in &group.items {
                all.extend(flatten_use_tree(item, prefix.clone())?);
            }
            Some(all)
        }
        UseTree::Glob(_) => None,
        UseTree::Rename(rename) => {
            let mut next = prefix;
            next.push(rename.ident.to_string());
            Some(vec![next])
        }
    }
}
