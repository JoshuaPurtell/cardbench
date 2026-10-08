//! Full-game PvP authority for RAV Coalhauler Swine.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, TriggerCondition, TriggeredAbilityBinding, Zone,
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
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-COALHAULER-SWINE";
const CARD_NAME: &str = "Coalhauler Swine";
const TRIGGER_ID: &str = "damage-each-player";
const FANGTAIL_ID: &str = "RAV-VIASHINO-FANGTAIL";
const FANGTAIL_ABILITY: &str = "tap-deal-one-to-player-or-creature";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs";
const REFERENCE_SEED: u64 = 0;
const MAX_REFERENCE_SEED: u64 = 64;
const MAX_TURNS: u32 = 50;
const MAX_POLICY_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs"]
mod candidate_coalhauler_swine;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Coalhauler Swine result: {error}");
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
            eprintln!("Coalhauler Swine PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let catalog = unique_catalog()?;
    let seed = reference_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Coalhauler Swine full-game replay was not deterministic".to_owned());
    }
    let mutations = [
        ("trigger_removed_detected_in_game", Mutation::RemoveTrigger),
        (
            "wrong_received_damage_condition_detected_in_game",
            Mutation::WrongCondition,
        ),
        (
            "fixed_damage_amount_detected_in_game",
            Mutation::WrongDamageAmount,
        ),
        ("wrong_mana_cost_detected_in_game", Mutation::WrongManaCost),
        ("wrong_base_stats_detected_in_game", Mutation::WrongStats),
    ];
    let mut sensitivity = serde_json::Map::new();
    for (name, mutation) in mutations {
        let detected = mutation_detected(seed, mutation)?;
        if !detected {
            return Err(format!(
                "Coalhauler Swine semantic mutation escaped: {name}"
            ));
        }
        sensitivity.insert(name.to_owned(), Value::Bool(true));
    }
    sensitivity.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 119, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority and casting/activation windows",
                "120 Damage and damage events",
                "302 Creature characteristics",
                "601.2 Casting spells and paying costs",
                "602.2 Activating abilities",
                "603 Triggered abilities",
                "608.2 Resolving spells and abilities",
                "704.5 State-based actions"
            ],
            "source_contract": "RAV #119 is a red 4/4 creature costing {4}{R}{R}; its mandatory received-damage trigger propagates the captured amount to each surviving player",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "mutation_sensitivity": sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveTrigger,
    WrongCondition,
    WrongDamageAmount,
    WrongManaCost,
    WrongStats,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, Option<TriggeredAbilityBinding>) {
    let mut definition = candidate_coalhauler_swine::definition();
    let mut trigger = Some(candidate_coalhauler_swine::triggered_ability());
    match mutation {
        Mutation::None => {}
        Mutation::RemoveTrigger => trigger = None,
        Mutation::WrongCondition => {
            if let Some(binding) = &mut trigger {
                binding.ability.condition = TriggerCondition::DealsDamage;
            }
        }
        Mutation::WrongDamageAmount => {
            if let Some(binding) = &mut trigger {
                binding.ability.effects = vec![Effect::DealDamageToEachPlayer { amount: 2 }];
            }
        }
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(3, [Color::Red, Color::Red]);
        }
        Mutation::WrongStats => definition.power = Some(3),
    }
    (definition, trigger)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, trigger) = candidate_values(Mutation::None);
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Coalhauler Swine is missing from the executable RAV registry")?;
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Coalhauler Swine damage trigger is missing from the RAV registry")?;
    if definition != registered || Some(registered_trigger) != trigger {
        return Err(
            "Coalhauler Swine definition/trigger are not delegated to its candidate module"
                .to_owned(),
        );
    }
    let trigger = trigger.expect("candidate trigger is present");
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(4, [Color::Red, Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "damage-received-to-each-player",
            ]
        || definition.power != Some(4)
        || definition.toughness != Some(4)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || trigger.card_definition != CARD_ID
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition != TriggerCondition::ReceivesDamage
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::DealDamageToEachPlayerFromReceivedDamage]
    {
        return Err(
            "Coalhauler Swine candidate differs from the RAV #119 feature contract".to_owned(),
        );
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    swine: Option<ObjectId>,
    fangtail: Option<ObjectId>,
    swine_stats: Option<(i32, i32)>,
    swine_payment: Vec<Color>,
    fangtail_payment: Vec<Color>,
    trigger_incarnation: Option<u64>,
    life_before_trigger: Option<[i64; 2]>,
    life_after_trigger: Option<[i64; 2]>,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, trigger) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Coalhauler Swine is missing from the RAV registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(trigger) = trigger {
        triggers.push(trigger);
    }
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
    let p0 = DeckList {
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
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: FANGTAIL_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (seat, deck) in [(0, &p0), (1, &p1)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!(
                "seat {seat} constructed deck has {count} cards, expected 60"
            ));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
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
    let mut policies = [
        Box::new(SwinePolicy::new(PlayerId(0))),
        Box::new(SwinePolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut trigger_incarnation = None;
    let mut life_before_trigger = None;
    let mut life_after_trigger = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_POLICY_MOVES {
            return Err("Coalhauler Swine match reached its bounded run limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Coalhauler policy seat missing")?;
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
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Coalhauler policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[event_start..] {
            match event {
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability: TRIGGER_ID,
                    ..
                } if game
                    .card_definition(*source)
                    .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    trigger_incarnation = Some(*source_incarnation);
                    life_before_trigger = Some([game.players[0].life, game.players[1].life]);
                }
                GameEvent::AbilityResolved {
                    source,
                    ability: TRIGGER_ID,
                    ..
                } if game
                    .card_definition(*source)
                    .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    life_after_trigger = Some([game.players[0].life, game.players[1].life]);
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let swine = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|item| item.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let fangtail = events.iter().find_map(|event| match event {
        GameEvent::AbilityActivated {
            player: PlayerId(1),
            source,
            definition,
            ability,
            ..
        } if *definition == FANGTAIL_ID && *ability == FANGTAIL_ABILITY => Some(*source),
        _ => None,
    });
    let swine_payment = swine
        .and_then(|object| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == object => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let fangtail_payment = fangtail
        .and_then(|object| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if *card == object => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let swine_stats = swine
        .and_then(|object| game.characteristics(object).ok())
        .and_then(|state| Some((state.power?, state.toughness?)));
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        swine,
        fangtail,
        swine_stats,
        swine_payment,
        fangtail_payment,
        trigger_incarnation,
        life_before_trigger,
        life_after_trigger,
    })
}

struct SwinePolicy {
    player: PlayerId,
    swine_cast_offered: bool,
    fangtail_cast_offered: bool,
    ping_offered: bool,
}

impl SwinePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            swine_cast_offered: false,
            fangtail_cast_offered: false,
            ping_offered: false,
        }
    }

    fn cast_action(
        &mut self,
        view: &GameView,
        card_id: &str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        let mut available: std::collections::BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        let mut can_pay = true;
        for color in &cost.colored {
            match available.get_mut(color) {
                Some(amount) if *amount > 0 => *amount -= 1,
                _ => can_pay = false,
            }
        }
        let mut generic = Vec::new();
        if can_pay && cost.hybrid.is_empty() {
            for _ in 0..cost.generic {
                let Some(color) = available
                    .iter()
                    .filter(|(_, count)| **count > 0)
                    .max_by_key(|(_, count)| **count)
                    .map(|(color, _)| *color)
                else {
                    can_pay = false;
                    break;
                };
                *available.get_mut(&color)? -= 1;
                generic.push(color);
            }
        } else {
            can_pay = false;
        }
        if can_pay {
            if card_id == CARD_ID {
                self.swine_cast_offered = true;
            }
            if card_id == FANGTAIL_ID {
                self.fangtail_cast_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic,
                    hybrid: vec![],
                },
            });
        }
        let missing_color = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let required =
            u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
        let wanted = missing_color.or_else(|| {
            (u16::from(view.mana_pool.total()) < required)
                .then(|| cost.colored.first().copied().unwrap_or(Color::Red))
        });
        let land = view.own_battlefield.iter().find(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && wanted.map_or(!land.mana_colors.is_empty(), |color| {
                    land.mana_colors.contains(&color)
                })
        })?;
        let color = wanted
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}

impl CodePolicy for SwinePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.coalhauler-swine-cast-trigger-win.v1"
        } else {
            "rav.coalhauler-swine-fangtail-source-damage.v1"
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
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    vec![]
                },
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if self.player == PlayerId(1) && !self.ping_offered && view.priority == self.player {
            let source = view.own_battlefield.iter().find(|card| {
                card.definition == Some(FANGTAIL_ID) && !card.tapped && !card.summoning_sick
            });
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(CARD_ID) && card.card_types.contains(&CardType::Creature)
            });
            if let (Some(source), Some(target)) = (source, target) {
                self.ping_offered = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: FANGTAIL_ABILITY,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) && !self.swine_cast_offered {
            if let Some(action) = self.cast_action(
                view,
                CARD_ID,
                &ManaCost::with_colors(4, [Color::Red, Color::Red]),
            ) {
                return action;
            }
        }
        if self.player == PlayerId(1) && !self.fangtail_cast_offered {
            if let Some(action) = self.cast_action(
                view,
                FANGTAIL_ID,
                &ManaCost::with_colors(2, [Color::Red, Color::Red]),
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Coalhauler Swine duel did not end in a clean seat-zero win".to_owned());
    }
    let swine = captured
        .swine
        .ok_or("seat zero did not cast Coalhauler Swine")?;
    let fangtail = captured
        .fangtail
        .ok_or("seat one did not activate Viashino Fangtail")?;
    if captured.swine_stats != Some((4, 4))
        || captured.swine_payment.len() != 6
        || captured
            .swine_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            < 2
        || captured.fangtail_payment.len() != 4
        || captured
            .fangtail_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            < 2
        || captured.life_before_trigger != Some([20, 20])
        || captured.life_after_trigger != Some([19, 19])
    {
        return Err(format!(
            "Swine characteristics/payment or equal player damage differs: stats={:?}, swine_payment={:?}, fangtail_payment={:?}, life={:?}",
            captured.swine_stats,
            captured.swine_payment,
            captured.fangtail_payment,
            captured.life_after_trigger
        ));
    }
    let events = &captured.events;
    let fangtail_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == fangtail && colors.len() == 4 && colors.iter().filter(|color| **color == Color::Red).count() >= 2)).ok_or("Fangtail exact cast payment receipt missing")?;
    let fangtail_cast = find_event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == fangtail)).ok_or("Fangtail SpellCast missing")?;
    let fangtail_resolved = find_event_after(
        events,
        fangtail_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == fangtail),
    )
    .ok_or("Fangtail SpellResolved missing")?;
    let fangtail_entered = find_event_after(events, fangtail_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == fangtail)).ok_or("Fangtail battlefield entry missing")?;
    let swine_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == swine && colors.len() == 6 && colors.iter().filter(|color| **color == Color::Red).count() >= 2)).ok_or("Swine exact {4}{R}{R} payment missing")?;
    let swine_cast = find_event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == swine)).ok_or("Swine SpellCast missing")?;
    let swine_resolved = find_event_after(
        events,
        swine_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == swine),
    )
    .ok_or("Swine SpellResolved missing")?;
    let swine_entered = find_event_after(events, swine_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == swine)).ok_or("Swine battlefield entry missing")?;
    if fangtail_paid >= fangtail_cast || swine_paid >= swine_cast {
        return Err("spell payment must precede each cast".to_owned());
    }
    let activated = find_event_after(events, fangtail_entered, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(1), source, definition, ability, .. } if *source == fangtail && *definition == FANGTAIL_ID && *ability == FANGTAIL_ABILITY)).ok_or("Fangtail activation receipt missing")?;
    let received = find_event_after(events, activated, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == fangtail && *permanent == swine)).ok_or("Fangtail did not deal exactly one damage to Swine")?;
    let stacked = find_event_after(events, received, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, source_incarnation, ability: TRIGGER_ID } if *source == swine && Some(*source_incarnation) == captured.trigger_incarnation)).ok_or("mandatory Swine received-damage trigger did not stack from the exact incarnation")?;
    let resolved = find_event_after(events, stacked, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: TRIGGER_ID } if *source == swine && Some(*source_incarnation) == captured.trigger_incarnation)).ok_or("Swine damage trigger did not resolve")?;
    let player_damage: Vec<_> = events.iter().enumerate().filter_map(|(index,event)| {
        if index <= stacked || index >= resolved { return None; }
        matches!(event, GameEvent::DamageDealtToPlayer { source, amount: 1, .. } if *source == swine).then_some((index,event))
    }).collect();
    let mut seats: Vec<_> = player_damage
        .iter()
        .filter_map(|(_, event)| match event {
            GameEvent::DamageDealtToPlayer { player, .. } => Some(player.0),
            _ => None,
        })
        .collect();
    seats.sort_unstable();
    if player_damage.len() != 2 || seats != [0, 1] {
        return Err(format!(
            "trigger must deal exactly the captured one damage to both living players, got {seats:?}"
        ));
    }
    let attack = find_event_after(events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&swine))).ok_or("Swine did not attack after its trigger resolved")?;
    let ended = find_event_after(events, attack, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal win after Swine attack missing")?;
    let mut selected = vec![
        fangtail_paid,
        fangtail_cast,
        fangtail_resolved,
        fangtail_entered,
        swine_paid,
        swine_cast,
        swine_resolved,
        swine_entered,
        activated,
        received,
        stacked,
    ];
    selected.extend(player_damage.iter().map(|(index, _)| *index));
    selected.extend([resolved, attack, ended]);
    selected.sort_unstable();
    let projected = project_selected_events(events, &selected);
    let kinds: Vec<_> = projected
        .iter()
        .map(|event| event["kind"].as_str().unwrap_or_default())
        .collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityActivated",
        "DamageDealtToPermanent",
        "TriggeredAbilityStacked",
        "DamageDealtToPlayer",
        "DamageDealtToPlayer",
        "AbilityResolved",
        "AttackersDeclared",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Coalhauler ordered receipt sequence differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-coalhauler-swine-received-damage-to-each-player-pvp",
        "seed": seed,
        "decks": ["rav-coalhauler-swine-mountains", "rav-viashino-fangtail-mountains"],
        "policies": ["rav.coalhauler-swine-cast-trigger-win.v1", "rav.coalhauler-swine-fangtail-source-damage.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "events_digest": event_digest(events),
        "mechanics": {
            "swine_object": swine.0,
            "swine_mana_cost": "{4}{R}{R}",
            "swine_mana_paid": captured.swine_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "swine_characteristics": {"power":4,"toughness":4},
            "damage_source_object": fangtail.0,
            "damage_source_controller_seat": 1,
            "damage_source_ability": FANGTAIL_ABILITY,
            "received_damage_amount": 1,
            "trigger_id": TRIGGER_ID,
            "trigger_controller_seat": 0,
            "trigger_source_incarnation": captured.trigger_incarnation,
            "trigger_stacked": true,
            "trigger_resolved": true,
            "player_damage": [{"seat":0,"amount":1},{"seat":1,"amount":1}],
            "life_totals_before_trigger": captured.life_before_trigger,
            "life_totals_after_trigger": captured.life_after_trigger,
            "fangtail_mana_paid": captured.fangtail_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "ordered_event_sequence": selected
        },
        "events": projected,
        "passed": true
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
        return Err(
            "RAV catalog identity/printing denominator differs from the task contract".to_owned(),
        );
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 119;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
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
fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<Value> {
    let projected = project_events(events);
    indices.iter().filter_map(|index| projected.get(*index)).map(|event: &ProjectedEvent| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})).collect()
}
fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let hash = bytes.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn reference_seed() -> Result<u64, String> {
    let seed = match std::env::var("CARDBENCH_COALHAULER_SWINE_SEED") {
        Ok(value) => value.parse::<u64>().map_err(|_| {
            "CARDBENCH_COALHAULER_SWINE_SEED must be an unsigned integer".to_owned()
        })?,
        Err(std::env::VarError::NotPresent) => REFERENCE_SEED,
        Err(error) => return Err(format!("could not read Coalhauler Swine seed: {error}")),
    };
    if seed > MAX_REFERENCE_SEED {
        return Err(format!(
            "Coalhauler Swine seed {seed} exceeds bounded maximum {MAX_REFERENCE_SEED}"
        ));
    }
    Ok(seed)
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
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Coalhauler Swine source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Coalhauler Swine source is not declarative Rust".to_owned())?;
    let mut funcs = BTreeSet::new();
    let mut engine_import = false;
    let mut collections_import = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Coalhauler Swine source has unsupported imports".to_owned());
                }
                for path in paths {
                    if path
                        .first()
                        .is_some_and(|part| part == "cardbench_magic_engine")
                    {
                        engine_import = true;
                    }
                    if path == ["std", "collections", "BTreeSet"] {
                        collections_import = true;
                    }
                }
            }
            Item::Fn(function) => {
                if !function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                }) || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                {
                    return Err(
                        "Coalhauler factories must be direct crate-visible literal factories"
                            .to_owned(),
                    );
                }
                let (name, ty) = match function.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                    _ => return Err("Coalhauler source has an unsupported function".to_owned()),
                };
                let direct = matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, ty));
                if function.sig.ident != name
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, output) if type_is(output, ty))
                    || !direct
                    || !funcs.insert(name.to_owned())
                {
                    return Err("Coalhauler factories must return direct typed literals".to_owned());
                }
            }
            _ => return Err("Coalhauler source contains an unsupported top-level item".to_owned()),
        }
    }
    if file.items.len() != 4
        || !engine_import
        || !collections_import
        || funcs != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
    {
        return Err("Coalhauler module structure differs from the task contract".to_owned());
    }
    Ok(())
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type_path(&path.path, name))
}
fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == name
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
