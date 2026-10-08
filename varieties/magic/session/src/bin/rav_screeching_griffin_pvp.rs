//! Full-game RAV Screeching Griffin authority for Flying and its blocking restriction.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, Color, CombatBlock, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
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
const CARD_ID: &str = "RAV-SCREECHING-GRIFFIN";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/screeching_griffin.rs";
const P0_DECK: &str = "rav_screeching_griffin_flying_control";
const P1_DECK: &str = "rav_screeching_griffin_skyknight_defense";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/screeching_griffin.rs"]
mod candidate_screeching_griffin;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Screeching Griffin PvP result: {error}");
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
            eprintln!("Screeching Griffin PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = 0;
    let captured = run_match(
        candidate_screeching_griffin::definition(),
        candidate_screeching_griffin::activated_ability(),
        seed,
        false,
    )?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(
        candidate_screeching_griffin::definition(),
        candidate_screeching_griffin::activated_ability(),
        seed,
        false,
    )?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.griffin != replay.griffin
        || captured.skyknight != replay.skyknight
        || captured.target != replay.target
    {
        return Err("Screeching Griffin full-game replay was not deterministic".to_owned());
    }
    let mutation_detected = blocking_restriction_mutation_fails(seed)?;
    if !mutation_detected {
        return Err(
            "removing Screeching Griffin's blocking restriction escaped the PvP trace".to_owned(),
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
        "card": {"collector_number": 29, "definition_id": CARD_ID, "name": "Screeching Griffin"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: white identity and colored payment",
                "117: priority to activate an ability and priority after activation",
                "302.1-302.6: creature permanents, Flying, and summoning sickness",
                "508: declaring attackers",
                "509.1: declaring blockers and source-relative restrictions",
                "601.2 and 608.3a: casting and resolving creature spells",
                "602.1-602.2: activated-ability costs and stack resolution",
                "611.2a and 613: until-end-of-turn continuous effects",
                "702.9: Flying"
            ],
            "source_contract": "RAV Screeching Griffin identity, Flying, and candidate-owned source-relative blocking ability",
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
            "target_blocking_restriction_mutation_detected": mutation_detected,
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
    let (status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "definition_id": definition_id, "name": card.name, "engine_status": status, "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "credited": false})
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
    griffin: Option<ObjectId>,
    skyknight: Option<ObjectId>,
    target: Option<ObjectId>,
    block_attempt_seen: bool,
    block_attempt_rejected: bool,
    block_attempt_accepted: bool,
    attempt_event_count_before: Option<usize>,
    attempt_event_count_after: Option<usize>,
    block_rejection_reason: Option<String>,
    target_live_at_attempt: bool,
    target_is_flyer_at_attempt: bool,
    griffin_cast_payment: Vec<Color>,
    skyknight_cast_payment: Vec<Color>,
    ability_payment: Vec<Color>,
    griffin_live: bool,
    live_flying: bool,
    live_power_toughness: Option<(i32, i32)>,
}

fn run_match(
    definition: CardDefinition,
    ability: ActivatedAbility,
    seed: u64,
    preview: bool,
) -> Result<Captured, String> {
    let griffin_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let skyknight_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-SKYKNIGHT-LEGIONNAIRE".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(P0_DECK, &griffin_deck), (P1_DECK, &skyknight_deck)] {
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
    game.load_deck_into_library(PlayerId(0), &griffin_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &skyknight_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GriffinPolicy::new(PlayerId(0), definition.mana_cost)),
        Box::new(GriffinPolicy::new(
            PlayerId(1),
            card_definitions()
                .into_iter()
                .find(|card| card.id == "RAV-SKYKNIGHT-LEGIONNAIRE")
                .ok_or("Skyknight definition missing")?
                .mana_cost,
        )),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut block_attempt_seen = false;
    let mut block_attempt_rejected = false;
    let mut block_attempt_accepted = false;
    let mut attempt_event_count_before = None;
    let mut attempt_event_count_after = None;
    let mut block_rejection_reason = None;
    let mut target_live_at_attempt = false;
    let mut target_is_flyer_at_attempt = false;
    let mut griffin = None;
    let mut skyknight = None;
    let mut target = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Screeching Griffin match exceeded its development bound".to_owned());
        }
        if preview && game.turn > 20 {
            break;
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
                target = assignments.first().map(|block| block.blocker);
                griffin = assignments.first().map(|block| block.attacker);
            }
            target_live_at_attempt =
                target.is_some_and(|object| game.zone_of(object) == Some(Zone::Battlefield));
            target_is_flyer_at_attempt = target.is_some_and(|object| {
                game.characteristics(object)
                    .is_ok_and(|c| c.keywords.contains(&Keyword::Flying))
            });
        }
        attempted += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => {
                accepted += 1;
                if challenge {
                    block_attempt_accepted = true;
                    attempt_event_count_after = Some(game.event_log.len());
                    if preview {
                        break;
                    }
                }
            }
            Err(error) if challenge => {
                block_attempt_rejected = true;
                block_rejection_reason = Some(error.to_string());
                attempt_event_count_after = Some(game.event_log.len());
                if attempt_event_count_before != attempt_event_count_after {
                    return Err(
                        "Screeching Griffin rejected block attempt changed the event log"
                            .to_owned(),
                    );
                }
                if preview {
                    break;
                }
            }
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    griffin = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
            _ => None,
        })
        .or(griffin);
    skyknight = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == "RAV-SKYKNIGHT-LEGIONNAIRE") =>
            {
                Some(*card)
            }
            _ => None,
        })
        .or(skyknight);
    let first_target = griffin.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::ContinuousEffectCreated {
                source: effect_source,
                target: effect_target,
                ..
            } if *effect_source == source => Some(*effect_target),
            _ => None,
        })
    });
    let target = first_target.or(target);
    let griffin_cast_payment = spell_payment(&events, PlayerId(0), griffin);
    let skyknight_cast_payment = spell_payment(&events, PlayerId(1), skyknight);
    let ability_payment = griffin
        .and_then(|source| {
            events.iter().find_map(|event| match event {
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source: paid_source,
                    ability: "prevent-target-blocking-griffin",
                    mana_cost,
                } if *paid_source == source
                    && *mana_cost == ManaCost::with_colors(0, [Color::Red]) =>
                {
                    Some(vec![Color::Red])
                }
                _ => None,
            })
        })
        .unwrap_or_default();
    let characteristics = griffin.and_then(|object| game.characteristics(object).ok());
    let live_flying = characteristics
        .as_ref()
        .is_some_and(|c| c.keywords.contains(&Keyword::Flying));
    let live_power_toughness = characteristics.and_then(|c| Some((c.power?, c.toughness?)));
    let griffin_live =
        griffin.is_some_and(|object| game.zone_of(object) == Some(Zone::Battlefield));
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: if preview && !game.is_game_over() {
            "InProgress".to_owned()
        } else {
            winner.map_or_else(
                || "Draw".to_owned(),
                |seat| format!("Winner(PlayerId({seat}))"),
            )
        },
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        griffin,
        skyknight,
        target,
        block_attempt_seen,
        block_attempt_rejected,
        block_attempt_accepted,
        attempt_event_count_before,
        attempt_event_count_after,
        block_rejection_reason,
        target_live_at_attempt,
        target_is_flyer_at_attempt,
        griffin_cast_payment,
        skyknight_cast_payment,
        ability_payment,
        griffin_live,
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
        .ok_or("Screeching Griffin definition is missing")?;
    *registered = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| {
            binding.card_definition == CARD_ID
                && binding.ability.id == "prevent-target-blocking-griffin"
        })
        .ok_or("Screeching Griffin binding is missing")?;
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

struct GriffinPolicy {
    player: PlayerId,
    cost: ManaCost,
    is_griffin_player: bool,
    cast_once: bool,
    challenged: bool,
    ability_turn: Option<u32>,
}

impl GriffinPolicy {
    fn new(player: PlayerId, cost: ManaCost) -> Self {
        Self {
            player,
            cost,
            is_griffin_player: player == PlayerId(0),
            cast_once: false,
            challenged: false,
            ability_turn: None,
        }
    }
    fn choose_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let red = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Red)
            })
            .count();
        let white = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::White)
            })
            .count();
        let preferred = if red <= white {
            Color::Red
        } else {
            Color::White
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

impl CodePolicy for GriffinPolicy {
    fn id(&self) -> &'static str {
        if self.is_griffin_player {
            "rav.screeching-griffin-ability-and-attack.v1"
        } else {
            "rav.screeching-griffin-skyknight-defense.v1"
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
            let attackers = if self.is_griffin_player {
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
        if !self.is_griffin_player
            && view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.challenged {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID));
                let blocker = view.own_battlefield.iter().find(|card| {
                    card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE") && card.can_block
                });
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
        if self.is_griffin_player {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            if let Some(source) =
                source.filter(|card| card.can_attack && self.ability_turn != Some(view.turn))
            {
                let target = view.opponent_battlefield.iter().find(|card| {
                    card.definition == Some("RAV-SKYKNIGHT-LEGIONNAIRE")
                        && card.card_types.contains(&CardType::Creature)
                });
                if let Some(target) = target {
                    if view.mana_pool.amount(Color::Red) > 0 {
                        self.ability_turn = Some(view.turn);
                        return PolicyAction::ActivateAbility {
                            activation: AbilityActivation {
                                source: source.id,
                                ability_id: "prevent-target-blocking-griffin",
                                sacrifice_sources: vec![],
                                additional_tap_creatures: vec![],
                                discard_cards: vec![],
                                targets: vec![Target::Permanent(target.id)],
                            },
                        };
                    }
                    if let Some(action) = self.tap_for(view, Color::Red) {
                        return action;
                    }
                }
            }
            if !self.cast_once {
                if let Some(action) = self.cast_action(view, CARD_ID, &self.cost.clone()) {
                    return action;
                }
            }
        } else if !self.cast_once {
            if let Some(action) =
                self.cast_action(view, "RAV-SKYKNIGHT-LEGIONNAIRE", &self.cost.clone())
            {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let griffin = captured.griffin.ok_or("Screeching Griffin was not cast")?;
    let skyknight = captured
        .skyknight
        .ok_or("opposing Skyknight Legionnaire was not cast")?;
    let target = captured
        .target
        .ok_or("Screeching Griffin did not target the opposing flyer")?;
    if target != skyknight || target == griffin {
        return Err("Screeching Griffin targeted the wrong creature".to_owned());
    }
    if captured.block_attempt_seen != true
        || !captured.block_attempt_rejected
        || captured.block_attempt_accepted
    {
        return Err(
            "Screeching Griffin did not prevent Skyknight Legionnaire from blocking it".to_owned(),
        );
    }
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Screeching Griffin PvP game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if captured.rejected_moves != 1
        || captured.attempt_event_count_before != captured.attempt_event_count_after
    {
        return Err(
            "Screeching Griffin block rejection was not a single event-atomic invalid move"
                .to_owned(),
        );
    }
    if !captured.target_live_at_attempt || !captured.target_is_flyer_at_attempt {
        return Err("the challenged Skyknight must be a live Flying blocker".to_owned());
    }
    if !captured.griffin_live
        || !captured.live_flying
        || captured.live_power_toughness != Some((2, 2))
    {
        return Err("Screeching Griffin must remain a live 2/2 with Flying".to_owned());
    }
    let white_griffin = captured
        .griffin_cast_payment
        .iter()
        .filter(|color| **color == Color::White)
        .count();
    if white_griffin < 1 || captured.griffin_cast_payment.len() != 4 {
        return Err(format!(
            "Screeching Griffin cast paid {:?}; one white plus three generic mana is required",
            captured.griffin_cast_payment
        ));
    }
    let red_skyknight = captured
        .skyknight_cast_payment
        .iter()
        .filter(|color| **color == Color::Red)
        .count();
    let white_skyknight = captured
        .skyknight_cast_payment
        .iter()
        .filter(|color| **color == Color::White)
        .count();
    if red_skyknight < 1 || white_skyknight < 1 || captured.skyknight_cast_payment.len() != 3 {
        return Err(format!(
            "opposing Skyknight cast paid {:?}; red, white, and one generic mana are required",
            captured.skyknight_cast_payment
        ));
    }
    if captured.ability_payment != [Color::Red] {
        return Err(format!(
            "Screeching Griffin ability payment was {:?}; one red mana is required",
            captured.ability_payment
        ));
    }
    let created = event_index(&captured.events, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: effect_target, .. } if *source == griffin && *effect_target == skyknight)).ok_or("Screeching Griffin source-relative restriction receipt missing")?;
    let activated = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition, ability: "prevent-target-blocking-griffin", .. } if *source == griffin && *definition == CARD_ID)).ok_or("Screeching Griffin AbilityActivated receipt missing")?;
    let resolved = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: "prevent-target-blocking-griffin", .. } if *source == griffin)).ok_or("Screeching Griffin AbilityResolved receipt missing")?;
    let ability_paid = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: "prevent-target-blocking-griffin", mana_cost } if *source == griffin && *mana_cost == ManaCost::with_colors(0, [Color::Red]))).ok_or("Screeching Griffin AbilityManaPaid receipt missing")?;
    let griffin_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == griffin)).ok_or("Screeching Griffin SpellCast receipt missing")?;
    let griffin_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == griffin),
    )
    .ok_or("Screeching Griffin SpellResolved receipt missing")?;
    let attack = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&griffin))).ok_or("Screeching Griffin did not attack")?;
    let empty_blockers = event_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("legal empty blocker declaration after Griffin's rejected block challenge missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == griffin)).ok_or("Screeching Griffin combat damage missing")?;
    let end = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded receipt missing")?;
    if !(griffin_cast < griffin_resolved
        && griffin_resolved < ability_paid
        && ability_paid < activated
        && activated < created
        && created < resolved
        && resolved < attack
        && attack < empty_blockers
        && empty_blockers < damage
        && damage < end)
    {
        return Err("Screeching Griffin cast, activation, blocking restriction, combat, and terminal event order differs".to_owned());
    }
    let events = relevant_events(&captured.events, griffin, skyknight)?;
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
        "AbilityManaPaid",
        "AbilityActivated",
        "ContinuousEffectCreated",
        "AbilityResolved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Screeching Griffin ordered projected events differ: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-screeching-griffin-blocks-skyknight-with-activated-restriction",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.screeching-griffin-ability-and-attack.v1", "rav.screeching-griffin-skyknight-defense.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "screeching_griffin_object": griffin.0,
        "skyknight_legionnaire_object": skyknight.0,
        "ability_target_object": target.0,
        "griffin_cast_payment": {"required": {"white_symbols": 1, "generic_symbols": 3, "total_symbols": 4}, "spent": {"white": white_griffin, "total": captured.griffin_cast_payment.len()}},
        "skyknight_cast_payment": {"required": {"red_symbols": 1, "white_symbols": 1, "generic_symbols": 1, "total_symbols": 3}, "spent": {"red": red_skyknight, "white": white_skyknight, "total": captured.skyknight_cast_payment.len()}},
        "ability_payment": {"required": {"red_symbols": 1, "total_symbols": 1}, "spent": {"red": captured.ability_payment.len()}},
        "live_flying": captured.live_flying,
        "resolved_power_toughness": {"power": 2, "toughness": 2},
        "block_attempt": {"target_is_skyknight": target == skyknight, "defender_controls_live_flyer": captured.target_live_at_attempt && captured.target_is_flyer_at_attempt, "rejected": captured.block_attempt_rejected, "engine_reason": captured.block_rejection_reason, "event_count_before": captured.attempt_event_count_before, "event_count_after": captured.attempt_event_count_after},
        "events": events,
        "passed": true
    }))
}

fn relevant_events(
    events: &[GameEvent],
    griffin: ObjectId,
    skyknight: ObjectId,
) -> Result<Vec<ProjectedEvent>, String> {
    let mut selected = Vec::new();
    for object in [griffin, skyknight] {
        let player = if object == griffin {
            PlayerId(0)
        } else {
            PlayerId(1)
        };
        for (index, event) in events.iter().enumerate() {
            let relevant = match event {
                GameEvent::SpellManaPaid {
                    player: p, card, ..
                } if *p == player && *card == object => Some("SpellManaPaid"),
                GameEvent::SpellCast { player: p, card } if *p == player && *card == object => {
                    Some("SpellCast")
                }
                GameEvent::SpellResolved { card } if *card == object => Some("SpellResolved"),
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                    ..
                } if *card == object => Some("CardMoved"),
                _ => None,
            };
            if relevant.is_some() {
                selected.push(index);
            }
        }
    }
    let mut ability_paid_selected = false;
    let mut ability_activated_selected = false;
    let mut effect_selected = false;
    let mut ability_resolved_selected = false;
    let mut attack_selected = false;
    let mut blockers_selected = false;
    let mut damage_selected = false;
    for (index, event) in events.iter().enumerate() {
        let relevant = match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: "prevent-target-blocking-griffin",
                ..
            } if *source == griffin && !ability_paid_selected => {
                ability_paid_selected = true;
                true
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                ability: "prevent-target-blocking-griffin",
                ..
            } if *source == griffin && !ability_activated_selected => {
                ability_activated_selected = true;
                true
            }
            GameEvent::ContinuousEffectCreated { source, target, .. }
                if *source == griffin && *target == skyknight && !effect_selected =>
            {
                effect_selected = true;
                true
            }
            GameEvent::AbilityResolved {
                source,
                ability: "prevent-target-blocking-griffin",
                ..
            } if *source == griffin && !ability_resolved_selected => {
                ability_resolved_selected = true;
                true
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if attackers.contains(&griffin) && !attack_selected => {
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
            } if *source == griffin && !damage_selected => {
                damage_selected = true;
                true
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        };
        if relevant {
            selected.push(index);
        }
    }
    selected.sort_unstable();
    selected.dedup();
    let selected: BTreeSet<usize> = selected.into_iter().collect();
    let relevant: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    if relevant.len() < 16 {
        return Err(format!(
            "Screeching Griffin event projection has only {} receipts",
            relevant.len()
        ));
    }
    Ok(relevant)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_digest(events: &[GameEvent]) -> String {
    let text = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let hash = text.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}
fn blocking_restriction_mutation_fails(seed: u64) -> Result<bool, String> {
    let mut ability = candidate_screeching_griffin::activated_ability();
    ability.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
        power: 1,
        toughness: 1,
    }];
    let captured = run_match(
        candidate_screeching_griffin::definition(),
        ability,
        seed,
        false,
    )?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("Screeching Griffin did not prevent Skyknight Legionnaire from blocking it")
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Screeching Griffin source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Screeching Griffin source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Screeching Griffin module attributes differ".to_owned());
    }
    let mut definition = None;
    let mut ability = None;
    let mut imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let paths = flatten_use_tree(&item.tree, Vec::new())
                    .ok_or("unsupported import in Screeching Griffin source")?;
                imports.extend(paths);
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item)
            }
            Item::Fn(item) if item.sig.ident == "activated_ability" && ability.is_none() => {
                ability = Some(item)
            }
            _ => return Err("Screeching Griffin source has executable top-level code".to_owned()),
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
        vec!["cardbench_magic_engine", "TargetRequirement"],
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
            "Screeching Griffin source must contain only its imports and two literal definitions"
                .to_owned(),
        );
    }
    validate_function(definition.unwrap(), "definition", "CardDefinition")?;
    validate_function(ability.unwrap(), "activated_ability", "ActivatedAbility")?;
    let card = literal_fields_from_function(
        file.items
            .iter()
            .find_map(|item| match item {
                Item::Fn(f) if f.sig.ident == "definition" => Some(f),
                _ => None,
            })
            .unwrap(),
        "CardDefinition",
    )?;
    check_string(card["id"], CARD_ID)?;
    check_string(card["name"], "Screeching Griffin")?;
    check_string(card["set_code"], "RAV")?;
    check_mana_cost(card["mana_cost"], 3, &[Color::White])?;
    check_set_paths(card["colors"], &[&["Color", "White"]])?;
    check_empty_set(card["mana_colors"])?;
    check_set_paths(card["card_types"], &[&["CardType", "Creature"]])?;
    check_bool(card["is_basic_land"], false)?;
    check_rules(card["supported_rules"])?;
    check_some_int(card["power"], 2)?;
    check_some_int(card["toughness"], 2)?;
    check_macro_tokens(card["keywords"], &["Keyword::Flying"], &[])?;
    check_empty_macro(card["effects"])?;
    let ability_fields = literal_fields_from_function(
        file.items
            .iter()
            .find_map(|item| match item {
                Item::Fn(f) if f.sig.ident == "activated_ability" => Some(f),
                _ => None,
            })
            .unwrap(),
        "ActivatedAbility",
    )?;
    check_string(ability_fields["id"], "prevent-target-blocking-griffin")?;
    check_mana_cost(ability_fields["mana_cost"], 0, &[Color::Red])?;
    check_bool(ability_fields["tap_cost"], false)?;
    check_bool(ability_fields["sorcery_speed"], false)?;
    check_int(ability_fields["additional_tap_creatures"], 0)?;
    check_bool(ability_fields["sacrifice_source"], false)?;
    check_int(ability_fields["sacrifice_creatures"], 0)?;
    check_int(ability_fields["sacrifice_lands"], 0)?;
    check_int(ability_fields["discard_cards"], 0)?;
    check_macro_tokens(
        ability_fields["targets"],
        &["TargetRequirement::Creature"],
        &[],
    )?;
    check_macro_tokens(
        ability_fields["effects"],
        &[
            "Effect::PreventTargetBlockingSourceUntilEndOfTurn",
            "Effect::ModifyTargetPtUntilEndOfTurn{power:1,toughness:1}",
        ],
        &[],
    )?;
    Ok(())
}

fn validate_function(function: &syn::ItemFn, name: &str, ty: &str) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|a| {
        is_doc_attribute(a) || a.path().is_ident("must_use") && matches!(a.meta, Meta::Path(_))
    });
    if !attrs_ok
        || !matches!(&function.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
        || function.sig.ident != name
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.block.stmts.len() != 1
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, output) if type_is(output, ty))
    {
        return Err(format!("Screeching Griffin {name} signature differs"));
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err(format!(
            "Screeching Griffin {name} must return one direct literal"
        ));
    };
    if !path_is(&value.path, &[ty]) || value.rest.is_some() {
        return Err(format!("Screeching Griffin {name} literal type differs"));
    }
    let expected_fields = if ty == "CardDefinition" { 13 } else { 11 };
    if literal_fields(value)?.len() != expected_fields {
        return Err(format!("Screeching Griffin {name} fields differ"));
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_screeching_griffin::definition();
    let registry = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Screeching Griffin is missing from the RAV registry")?;
    if candidate.id != registry.id
        || candidate.name != "Screeching Griffin"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(3, [Color::White])
        || candidate.colors != BTreeSet::from([Color::White])
        || candidate.card_types != BTreeSet::from([CardType::Creature])
        || candidate.power != Some(2)
        || candidate.toughness != Some(2)
        || candidate.keywords != [Keyword::Flying]
        || !candidate.effects.is_empty()
    {
        return Err("Screeching Griffin printed identity differs".to_owned());
    }
    let ability = candidate_screeching_griffin::activated_ability();
    if ability.id != "prevent-target-blocking-griffin"
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || !(ability.effects == [Effect::PreventTargetBlockingSourceUntilEndOfTurn]
            || ability.effects
                == [Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }])
    {
        return Err("Screeching Griffin activated binding differs".to_owned());
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
            Color::White => ["Color", "White"],
            Color::Red => ["Color", "Red"],
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
        "flying",
        "activated-prevent-target-blocking-source",
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
    let normalized = mac.mac.tokens.to_string().replace(' ', "");
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
