//! Full-game RAV Trophy Hunter authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, CounterKind,
    DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Target, TriggerCondition, TriggeredAbilityBinding,
    Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-TROPHY-HUNTER";
const CARD_NAME: &str = "Trophy Hunter";
const BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const ACTIVATED_ID: &str = "deal-one-damage-to-flying-creature";
const TRIGGER_ID: &str = "damaged-creature-dies-add-plus-one-counter";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/trophy_hunter.rs";
const REFERENCE_SEED: u64 = 0;
const MAX_REFERENCE_SEED: u64 = 64;
const MAX_TURNS: u32 = 50;
const MAX_POLICY_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/trophy_hunter.rs"]
mod candidate_trophy_hunter;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Trophy Hunter PvP result: {error}");
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
            eprintln!("Trophy Hunter PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_runtime()?;
    let catalog = unique_catalog()?;
    let seed = reference_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Trophy Hunter full-game replay was not deterministic".to_owned());
    }
    let mutations = [
        ("activated_damage_removed_detected", Mutation::RemoveDamage),
        ("activation_tap_cost_removed_detected", Mutation::NoTapCost),
        ("wrong_trigger_condition_detected", Mutation::WrongCondition),
        ("trigger_counter_removed_detected", Mutation::RemoveCounter),
        ("wrong_activated_cost_detected", Mutation::WrongAbilityCost),
        ("wrong_base_power_detected", Mutation::WrongPower),
    ];
    let mut sensitivity = serde_json::Map::new();
    for (name, mutation) in mutations {
        let detected = run_match(seed, mutation)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| ()))
            .is_err();
        if !detected {
            return Err(format!("Trophy Hunter semantic mutation escaped: {name}"));
        }
        sensitivity.insert(name.to_owned(), Value::Bool(true));
    }
    sensitivity.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":187,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "117 Priority and activation windows",
                "120 Damage and committed damage receipts",
                "302.1 Creature characteristics",
                "602.2 Activating abilities and paying costs",
                "603 Triggered abilities and event conditions",
                "608 Resolving abilities",
                "704.5 State-based actions and creature deaths"
            ],
            "source_contract":"RAV #187 Trophy Hunter is a green {2}{G} 2/3 creature with {1}{G}, tap: deal 1 damage to target creature with flying; when a creature damaged by this exact source incarnation this turn is put into a graveyard from the battlefield, put a +1/+1 counter on Trophy Hunter",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "deterministic_replay":true,
        "mutation_sensitivity":sensitivity,
        "candidate_source_validation":source_validation,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveDamage,
    WrongCondition,
    RemoveCounter,
    WrongAbilityCost,
    WrongPower,
    NoTapCost,
}

fn candidate_values(
    mutation: Mutation,
) -> (
    CardDefinition,
    ActivatedAbilityBinding,
    TriggeredAbilityBinding,
) {
    let mut definition = candidate_trophy_hunter::definition();
    let mut activation = candidate_trophy_hunter::activated_ability_binding();
    let mut trigger = candidate_trophy_hunter::triggered_ability_binding();
    match mutation {
        Mutation::None => {}
        Mutation::RemoveDamage => activation.ability.effects.clear(),
        Mutation::WrongCondition => trigger.ability.condition = TriggerCondition::Dies,
        Mutation::RemoveCounter => trigger.ability.effects.clear(),
        Mutation::WrongAbilityCost => {
            activation.ability.mana_cost = ManaCost::with_colors(0, [Color::Green]);
        }
        Mutation::WrongPower => definition.power = Some(3),
        Mutation::NoTapCost => activation.ability.tap_cost = false,
    }
    (definition, activation, trigger)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, activation, trigger) = candidate_values(Mutation::None);
    if definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(3)
        || definition.keywords.len() != 0
        || !definition.effects.is_empty()
        || activation.ability.id != ACTIVATED_ID
        || activation.ability.mana_cost != ManaCost::with_colors(1, [Color::Green])
        || !activation.ability.tap_cost
        || activation.ability.sorcery_speed
        || activation.ability.targets != [cardbench_magic_engine::TargetRequirement::FlyingCreature]
        || activation.ability.effects
            != [Effect::DealDamage {
                amount: 1,
                target: cardbench_magic_engine::TargetRequirement::FlyingCreature,
            }]
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition != TriggerCondition::DamagedCreatureBySourceDiesThisTurn
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::AddPlusOneCounterToSource]
    {
        return Err(
            "Trophy Hunter candidate differs from the RAV #187 feature contract".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    hunter: Option<ObjectId>,
    bird: Option<ObjectId>,
    hunter_stats: Option<(i32, i32)>,
    hunter_payment: Vec<Color>,
    ability_payment: Option<ManaCost>,
    hunter_incarnation_at_activation: Option<u64>,
    hunter_tapped_at_activation: bool,
    target_incarnation_at_activation: Option<u64>,
    trigger_source_incarnation: Option<u64>,
    target_post_move_incarnation: Option<u64>,
    counter_total: Option<i16>,
    target_zone: Option<Zone>,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, activation, trigger) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Trophy Hunter is missing from the RAV registry")? = definition;
    let activations = vec![activation];
    for binding in &activations {
        let effect_targets = binding
            .ability
            .effects
            .iter()
            .flat_map(|effect| effect.target_requirements().into_iter().flatten())
            .collect::<Vec<_>>();
        if effect_targets != binding.ability.targets {
            return Err(format!(
                "registered activation target/effect mismatch for {}:{} targets={:?} effect_targets={effect_targets:?}",
                binding.card_definition, binding.ability.id, binding.ability.targets
            ));
        }
    }
    let triggers = vec![trigger];
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activations,
        triggers,
    )
    .map_err(|error| error.to_string())?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BIRD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (seat, deck) in [(0, &pilot), (1, &opponent)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("seat {seat} deck has {count} cards, expected 60"));
        }
    }
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

    let mut policies = [
        Box::new(HunterPolicy::new(PlayerId(0))),
        Box::new(HunterPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut hunter_incarnation_at_activation = None;
    let mut hunter_tapped_at_activation = false;
    let mut target_incarnation_at_activation = None;
    let mut trigger_source_incarnation = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_POLICY_MOVES {
            let cast_cards: Vec<_> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::SpellCast { player, card } => game
                        .card_definition(*card)
                        .ok()
                        .map(|definition| (player.0, definition.id)),
                    _ => None,
                })
                .collect();
            let battlefield: Vec<_> = [PlayerId(0), PlayerId(1)]
                .into_iter()
                .map(|player| {
                    game.view_for_player(player).map(|view| {
                        (
                            view.own_battlefield
                                .iter()
                                .map(|card| (card.definition, card.tapped, card.can_attack))
                                .collect::<Vec<_>>(),
                            view.hand
                                .iter()
                                .filter_map(|card| card.definition)
                                .collect::<Vec<_>>(),
                            view.lands_played,
                        )
                    })
                })
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?;
            return Err(format!(
                "Trophy Hunter duel exceeded the bounded run limit at turn {} after {} policy moves (winner {:?}, life={:?}, casts={cast_cards:?}, battlefield_and_hands={battlefield:?}, recent_events={:?})",
                game.turn,
                accepted,
                game.winner(),
                [
                    game.player(PlayerId(0)).ok().map(|player| player.life),
                    game.player(PlayerId(1)).ok().map(|player| player.life)
                ],
                game.event_log.iter().rev().take(24).collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Trophy Hunter policy seat missing")?;
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
        let chosen_target = policy.last_target;
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Trophy Hunter policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[event_start..] {
            match event {
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    policies[0].hunter_entered_turn = Some(game.turn);
                }
                GameEvent::AbilityActivated {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } if *ability == ACTIVATED_ID
                    && game
                        .card_definition(*source)
                        .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    hunter_incarnation_at_activation = Some(*source_incarnation);
                    hunter_tapped_at_activation =
                        game.object(*source).is_ok_and(|object| object.tapped);
                    if let Some(target) = chosen_target {
                        target_incarnation_at_activation =
                            game.object(target).ok().map(|obj| obj.incarnation);
                    }
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } if *ability == TRIGGER_ID
                    && game
                        .card_definition(*source)
                        .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    trigger_source_incarnation = Some(*source_incarnation);
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let hunter = events.iter().find_map(|event| match event {
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
    let bird = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|item| item.id == BIRD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let hunter_payment = hunter
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
    let ability_payment = hunter.and_then(|object| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability,
                mana_cost,
            } if *source == object && *ability == ACTIVATED_ID => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let hunter_stats = hunter
        .and_then(|object| game.characteristics(object).ok())
        .and_then(|state| Some((state.power?, state.toughness?)));
    let counter_total = hunter
        .and_then(|object| game.object(object).ok())
        .map(|object| {
            object
                .counters
                .get(&CounterKind::PlusOnePlusOne)
                .copied()
                .unwrap_or(0)
        });
    let target_graveyard_move = bird.and_then(|object| {
        find_event(&events, |event| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == object)
        })
    });
    let target_zone = target_graveyard_move.map(|_| Zone::Graveyard);
    let target_post_move_incarnation = bird.and_then(|object| {
        let moved = target_graveyard_move?;
        find_event_after(&events, moved, |event| {
            matches!(event, GameEvent::ObjectIncarnationAdvanced { object: advanced, .. } if *advanced == object)
        })
        .and_then(|index| match events.get(index) {
            Some(GameEvent::ObjectIncarnationAdvanced { incarnation, .. }) => Some(*incarnation),
            _ => None,
        })
    });
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
        hunter,
        bird,
        hunter_stats,
        hunter_payment,
        ability_payment,
        hunter_incarnation_at_activation,
        hunter_tapped_at_activation,
        target_incarnation_at_activation,
        trigger_source_incarnation,
        target_post_move_incarnation,
        counter_total,
        target_zone,
    })
}

struct HunterPolicy {
    player: PlayerId,
    hunter_cast: bool,
    hunter_entered_turn: Option<u32>,
    bird_cast: bool,
    ability_activated: bool,
    last_target: Option<ObjectId>,
}

impl HunterPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            hunter_cast: false,
            hunter_entered_turn: None,
            bird_cast: false,
            ability_activated: false,
            last_target: None,
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
                    .filter(|(_, amount)| **amount > 0)
                    .max_by_key(|(_, amount)| **amount)
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
                self.hunter_cast = true;
            }
            if card_id == BIRD_ID {
                self.bird_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
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
                .then(|| cost.colored.first().copied().unwrap_or(Color::Green))
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

impl CodePolicy for HunterPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.trophy-hunter-damage-death-win.v1"
        } else {
            "rav.birds-of-paradise-flyer-target.v1"
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
        if self.player == PlayerId(0)
            && !self.ability_activated
            && self
                .hunter_entered_turn
                .is_some_and(|entered_turn| view.turn > entered_turn)
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(BIRD_ID)
                    && card.card_types.contains(&CardType::Creature)
                    && card
                        .keywords
                        .contains(&cardbench_magic_engine::Keyword::Flying)
            });
            if let (Some(source), Some(target)) = (source, target) {
                if view.mana_pool.amount(Color::Green) >= 1 && view.mana_pool.total() >= 2 {
                    self.ability_activated = true;
                    self.last_target = Some(target.id);
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ACTIVATED_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(target.id)],
                        },
                    };
                }
                if let Some(land) = view.own_battlefield.iter().find(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && land.mana_colors.contains(&Color::Green)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
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
                .find(|card| card.definition == Some("RAV-FOREST"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) && !self.hunter_cast {
            let lands = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count();
            if lands >= 5 {
                if let Some(action) =
                    self.cast_action(view, CARD_ID, &ManaCost::with_colors(2, [Color::Green]))
                {
                    return action;
                }
            }
        }
        if self.player == PlayerId(1) && !self.bird_cast {
            if let Some(action) =
                self.cast_action(view, BIRD_ID, &ManaCost::with_colors(0, [Color::Green]))
            {
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
        return Err("Trophy Hunter duel did not end in a clean seat-zero win".to_owned());
    }
    let hunter = captured
        .hunter
        .ok_or("seat zero did not cast Trophy Hunter")?;
    let bird = captured
        .bird
        .ok_or("seat one did not cast Birds of Paradise")?;
    if captured.hunter_payment.len() != 3
        || captured
            .hunter_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            < 1
        || captured.ability_payment.as_ref() != Some(&ManaCost::with_colors(1, [Color::Green]))
        || !captured.hunter_tapped_at_activation
        || captured.hunter_stats != Some((3, 4))
        || captured.counter_total != Some(1)
        || captured.target_zone != Some(Zone::Graveyard)
        || captured.hunter_incarnation_at_activation.is_none()
        || captured.target_incarnation_at_activation.is_none()
        || captured.target_post_move_incarnation == captured.target_incarnation_at_activation
        || captured.trigger_source_incarnation != captured.hunter_incarnation_at_activation
    {
        return Err(format!(
            "Trophy Hunter cast, activation, death, or counter facts differ: {captured:?}"
        ));
    }
    let events = &captured.events;
    let hunter_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == hunter && colors.len() == 3 && colors.contains(&Color::Green))).ok_or("Trophy Hunter exact {2}{G} payment receipt missing")?;
    let hunter_cast = find_event_after(events, hunter_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == hunter)).ok_or("Trophy Hunter SpellCast missing")?;
    let hunter_resolved = find_event_after(
        events,
        hunter_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == hunter),
    )
    .ok_or("Trophy Hunter SpellResolved missing")?;
    let hunter_entered = find_event_after(events, hunter_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == hunter)).ok_or("Trophy Hunter battlefield entry missing")?;
    let bird_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == bird && colors == &[Color::Green])).ok_or("Birds of Paradise exact {G} payment receipt missing")?;
    let bird_cast = find_event_after(events, bird_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == bird)).ok_or("Birds of Paradise SpellCast missing")?;
    let bird_resolved = find_event_after(
        events,
        bird_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == bird),
    )
    .ok_or("Birds of Paradise SpellResolved missing")?;
    let bird_entered = find_event_after(events, bird_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == bird)).ok_or("Birds of Paradise battlefield entry missing")?;
    let paid = find_event_after(events, hunter_entered, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability, mana_cost } if *source == hunter && *ability == ACTIVATED_ID && *mana_cost == ManaCost::with_colors(1, [Color::Green]))).ok_or("Trophy Hunter exact {1}{G} ability payment missing")?;
    let activation = find_event_after(events, paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, source_incarnation, definition, ability } if *source == hunter && *source_incarnation == captured.hunter_incarnation_at_activation.unwrap_or_default() && *definition == CARD_ID && *ability == ACTIVATED_ID)).ok_or("Trophy Hunter activation receipt is missing")?;
    let damage = find_event_after(events, activation, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == hunter && *permanent == bird)).ok_or("Trophy Hunter did not commit one damage to the selected flyer")?;
    let died = find_event_after(events, damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == bird)).ok_or("damaged flying creature did not move from battlefield to graveyard")?;
    let activated_resolved = find_event_after(events, damage, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: ACTIVATED_ID } if *source == hunter && Some(*source_incarnation) == captured.hunter_incarnation_at_activation)).ok_or("Trophy Hunter damage ability did not resolve")?;
    let stacked = find_event_after(events, died, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, source_incarnation, ability: TRIGGER_ID } if *source == hunter && Some(*source_incarnation) == captured.hunter_incarnation_at_activation)).ok_or("same-turn damaged-creature death trigger did not stack from Trophy Hunter's exact incarnation")?;
    let counter = find_event_after(events, stacked, |event| matches!(event, GameEvent::CounterPlaced { source, card, counter: CounterKind::PlusOnePlusOne, amount: 1 } if *source == hunter && *card == hunter)).ok_or("Trophy Hunter trigger did not place one +1/+1 counter on its source")?;
    let trigger_resolved = find_event_after(events, counter, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation, ability: TRIGGER_ID } if *source == hunter && Some(*source_incarnation) == captured.hunter_incarnation_at_activation)).ok_or("source-aware death trigger did not resolve")?;
    if paid >= activation
        || damage >= died
        || stacked >= trigger_resolved
        || activated_resolved >= stacked
    {
        return Err("Trophy Hunter event receipts were not ordered by the activation, damage, death, and trigger lifecycle".to_owned());
    }
    let attack = find_event_after(events, trigger_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&hunter))).ok_or("Trophy Hunter did not attack after its trigger resolved")?;
    let ended = find_event_after(events, attack, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal win after Trophy Hunter attack missing")?;
    let mut selected = vec![
        hunter_paid,
        hunter_cast,
        hunter_resolved,
        hunter_entered,
        bird_paid,
        bird_cast,
        bird_resolved,
        bird_entered,
        paid,
        activation,
        damage,
        died,
        activated_resolved,
        stacked,
        counter,
        trigger_resolved,
        attack,
        ended,
    ];
    selected.sort_unstable();
    let projected = project_events(events);
    let trace_events: Vec<Value> = selected.iter().map(|index| {
        let event = &projected[*index];
        json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})
    }).collect();
    let selected_indices: Vec<usize> = selected.to_vec();
    Ok(json!({
        "id":"rav-trophy-hunter-source-aware-death-counter-pvp",
        "seed":seed,
        "decks":["rav-trophy-hunter-forests","rav-birds-of-paradise-forests"],
        "policies":["rav.trophy-hunter-damage-death-win.v1","rav.birds-of-paradise-flyer-target.v1"],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted,
        "rejected_moves":captured.rejected,
        "events_digest":event_digest(events),
        "mechanics":{
            "hunter_object":hunter.0,
            "hunter_mana_cost":"{2}{G}",
            "hunter_mana_paid":captured.hunter_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "hunter_base_characteristics":{"power":2,"toughness":3},
            "hunter_characteristics_after_trigger":{"power":captured.hunter_stats.map(|stats|stats.0),"toughness":captured.hunter_stats.map(|stats|stats.1)},
            "activated_mana_cost":"{1}{G}",
            "activated_mana_paid":"{1}{G}",
            "activated_tap_cost_paid":captured.hunter_tapped_at_activation,
            "activated_source_incarnation":captured.hunter_incarnation_at_activation,
            "target_object":bird.0,
            "target_incarnation_at_activation":captured.target_incarnation_at_activation,
            "target_post_move_incarnation":captured.target_post_move_incarnation,
            "damage_amount":1,
            "target_zone_after_damage":"Graveyard",
            "trigger_id":TRIGGER_ID,
            "trigger_source_incarnation":captured.trigger_source_incarnation,
            "trigger_stacked":true,
            "trigger_resolved":true,
            "source_plus_one_counter_total":captured.counter_total,
            "ordered_event_sequence":selected_indices
        },
        "events":trace_events,
        "passed":true
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
            "RAV catalog identity/printing denominator differs from task contract".to_owned(),
        );
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 187;
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
    let seed = match std::env::var("CARDBENCH_TROPHY_HUNTER_SEED") {
        Ok(value) => value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_TROPHY_HUNTER_SEED must be an unsigned integer".to_owned())?,
        Err(std::env::VarError::NotPresent) => REFERENCE_SEED,
        Err(error) => return Err(format!("could not read Trophy Hunter seed: {error}")),
    };
    if seed > MAX_REFERENCE_SEED {
        return Err(format!(
            "Trophy Hunter seed {seed} exceeds bounded maximum {MAX_REFERENCE_SEED}"
        ));
    }
    Ok(seed)
}
fn output_root() -> Option<std::path::PathBuf> {
    std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
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

// Canonical candidate text is independent of the candidate path. Whitespace/comments may change,
// but token order and every literal remain pinned to the reviewed RAV #187 implementation.
const CANONICAL_CANDIDATE_SOURCE: &str = "usestd::collections::BTreeSet;usecardbench_magic_engine::{ActivatedAbility,ActivatedAbilityBinding,CardDefinition,CardType,Color,Effect,ManaCost,TargetRequirement,TriggerCondition,TriggeredAbility,TriggeredAbilityBinding,};#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:\"RAV-TROPHY-HUNTER\",name:\"Trophy Hunter\",set_code:\"RAV\",mana_cost:ManaCost::with_colors(2,[Color::Green]),colors:BTreeSet::from([Color::Green]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Creature]),is_basic_land:false,supported_rules:&[\"full-rules-fidelity\",\"colored-cost-casting\",\"base-characteristics\",\"activated-flying-targeted-one-damage\",\"source-dealt-damage-delayed-death-trigger\",],power:Some(2),toughness:Some(3),keywords:vec![],effects:vec![],}}#[must_use]pub(crate)fnactivated_ability_binding()->ActivatedAbilityBinding{ActivatedAbilityBinding{card_definition:\"RAV-TROPHY-HUNTER\",ability:ActivatedAbility{id:\"deal-one-damage-to-flying-creature\",mana_cost:ManaCost::with_colors(1,[Color::Green]),tap_cost:true,sorcery_speed:false,additional_tap_creatures:0,sacrifice_source:false,sacrifice_creatures:0,sacrifice_lands:0,discard_cards:0,targets:vec![TargetRequirement::FlyingCreature],effects:vec![Effect::DealDamage{amount:1,target:TargetRequirement::FlyingCreature,}],},}}#[must_use]pub(crate)fntriggered_ability_binding()->TriggeredAbilityBinding{TriggeredAbilityBinding{card_definition:\"RAV-TROPHY-HUNTER\",ability:TriggeredAbility{id:\"damaged-creature-dies-add-plus-one-counter\",condition:TriggerCondition::DamagedCreatureBySourceDiesThisTurn,mana_cost:ManaCost::new(0),optional:false,targets:vec![],effects:vec![Effect::AddPlusOneCounterToSource],},}}";
const INJECTED_SIDE_EFFECT: &str = "if std::process::id() > 0 { println!(\"injected candidate side effect\"); std::process::exit(0); }";

// This dead fixture makes the malicious insertion compile-valid in every authority build. It is
// never called; the source guard must reject the equivalent injected statement before candidate
// definition or ability functions run.
#[allow(dead_code, unreachable_code)]
fn compile_valid_injected_side_effect_fixture() -> CardDefinition {
    if std::process::id() > 0 {
        println!("injected candidate side effect");
        std::process::exit(0);
    }
    candidate_trophy_hunter::definition()
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Trophy Hunter source is not valid Rust".to_owned())?;
    if normalize_candidate_source(source)? != CANONICAL_CANDIDATE_SOURCE {
        return Err(
            "Trophy Hunter candidate differs from the RAV #187 feature contract: normalized source pin mismatch".to_owned(),
        );
    }
    if !candidate_ast_has_literal_shape(&file) {
        return Err(
            "Trophy Hunter candidate differs from the RAV #187 feature contract: source AST shape mismatch"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Trophy Hunter source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        &format!("pub(crate) fn definition() -> CardDefinition {{ {INJECTED_SIDE_EFFECT}"),
        1,
    );
    if injected == source || validate_candidate_source_text(&injected).is_ok() {
        return Err(
            "Trophy Hunter source guard failed to reject a compile-valid injected side effect"
                .to_owned(),
        );
    }
    Ok(json!({
        "normalized_literal_source_contract_matched":true,
        "ast_literal_shape_matched":true,
        "compile_valid_injected_side_effect_fixture_compiled":true,
        "compile_valid_injected_side_effect_candidate_rejected":true,
        "guard_ran_before_candidate_function_calls":true
    }))
}

fn normalize_candidate_source(source: &str) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_comment_depth = 0_usize;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    while index < chars.len() {
        let current = chars[index];
        if in_line_comment {
            if current == '\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment_depth > 0 {
            if current == '/' && chars.get(index + 1) == Some(&'*') {
                block_comment_depth += 1;
                index += 2;
            } else if current == '*' && chars.get(index + 1) == Some(&'/') {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string || in_char {
            normalized.push(current);
            if current == '\\' {
                if let Some(escaped) = chars.get(index + 1) {
                    normalized.push(*escaped);
                    index += 2;
                    continue;
                }
            } else if (in_string && current == '"') || (in_char && current == '\'') {
                in_string = false;
                in_char = false;
            }
            index += 1;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            in_line_comment = true;
            index += 2;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            block_comment_depth = 1;
            index += 2;
            continue;
        }
        if current == '"' {
            in_string = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if current == '\'' && candidate_has_closing_char_quote(&chars, index + 1) {
            in_char = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if !current.is_whitespace() {
            normalized.push(current);
        }
        index += 1;
    }
    if in_string || in_char || block_comment_depth > 0 {
        return Err("Trophy Hunter source has an unterminated literal or comment".to_owned());
    }
    Ok(normalized)
}

fn candidate_has_closing_char_quote(chars: &[char], mut index: usize) -> bool {
    while let Some(character) = chars.get(index) {
        if *character == '\n' {
            return false;
        }
        if *character == '\\' {
            index += 2;
            continue;
        }
        if *character == '\'' {
            return true;
        }
        index += 1;
    }
    false
}

fn candidate_ast_has_literal_shape(file: &syn::File) -> bool {
    use syn::{Expr, Item, Stmt, Visibility};

    if file.items.len() != 5
        || !matches!(file.items[0], Item::Use(_))
        || !matches!(file.items[1], Item::Use(_))
        || file
            .attrs
            .iter()
            .any(|attribute| !attribute.path().is_ident("doc"))
    {
        return false;
    }
    let (Some(Item::Fn(definition)), Some(Item::Fn(activation)), Some(Item::Fn(trigger))) =
        (file.items.get(2), file.items.get(3), file.items.get(4))
    else {
        return false;
    };
    let function_shape = |function: &syn::ItemFn, name: &str, result: &str, fields: &[&str]| {
        let visible = matches!(&function.vis, Visibility::Restricted(vis)
            if vis.path.is_ident("crate") && vis.in_token.is_none());
        let sig = &function.sig;
        visible
            && sig.ident == name
            && sig.inputs.is_empty()
            && sig.generics.params.is_empty()
            && sig.generics.where_clause.is_none()
            && sig.constness.is_none()
            && sig.asyncness.is_none()
            && sig.unsafety.is_none()
            && sig.abi.is_none()
            && sig.variadic.is_none()
            && candidate_return_type_is(&sig.output, result)
            && function.attrs.iter().all(|attribute| {
                attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
            })
            && function
                .attrs
                .iter()
                .filter(|attribute| attribute.path().is_ident("must_use"))
                .count()
                == 1
            && function.block.stmts.len() == 1
            && matches!(function.block.stmts.first(), Some(Stmt::Expr(Expr::Struct(expr), None))
                if candidate_struct_has_literal_shape(expr, result, fields))
    };
    let defs = function_shape(
        definition,
        "definition",
        "CardDefinition",
        &[
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
        ],
    );
    let act_function_shape = function_shape(
        activation,
        "activated_ability_binding",
        "ActivatedAbilityBinding",
        &["card_definition", "ability"],
    );
    let act_nested_shape = candidate_nested_ability_shape(
        activation,
        "ActivatedAbility",
        &[
            "id",
            "mana_cost",
            "tap_cost",
            "sorcery_speed",
            "additional_tap_creatures",
            "sacrifice_source",
            "sacrifice_creatures",
            "sacrifice_lands",
            "discard_cards",
            "targets",
            "effects",
        ],
    );
    let trig_function_shape = function_shape(
        trigger,
        "triggered_ability_binding",
        "TriggeredAbilityBinding",
        &["card_definition", "ability"],
    );
    let trig_nested_shape = candidate_nested_ability_shape(
        trigger,
        "TriggeredAbility",
        &[
            "id",
            "condition",
            "mana_cost",
            "optional",
            "targets",
            "effects",
        ],
    );
    defs && act_function_shape && act_nested_shape && trig_function_shape && trig_nested_shape
}

fn candidate_return_type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output, syn::ReturnType::Type(_, ty) if matches!(&**ty,
        syn::Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|s| s.ident == expected)))
}

fn candidate_nested_ability_shape(function: &syn::ItemFn, name: &str, fields: &[&str]) -> bool {
    let Some(syn::Stmt::Expr(syn::Expr::Struct(binding), None)) = function.block.stmts.first()
    else {
        return false;
    };
    let Some(ability) = binding.fields.iter().find_map(|field| match &field.member {
        syn::Member::Named(member) if member == "ability" => match &field.expr {
            syn::Expr::Struct(value) => Some(value),
            _ => None,
        },
        _ => None,
    }) else {
        return false;
    };
    candidate_struct_has_literal_shape(ability, name, fields)
}

fn candidate_struct_has_literal_shape(
    expression: &syn::ExprStruct,
    name: &str,
    fields: &[&str],
) -> bool {
    let actual: Vec<_> = expression
        .fields
        .iter()
        .map(|field| match &field.member {
            syn::Member::Named(ident) => Some(ident.to_string()),
            syn::Member::Unnamed(_) => None,
        })
        .collect();
    let expected: Vec<_> = fields
        .iter()
        .map(|field| Some((*field).to_owned()))
        .collect();
    expression
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
        && actual == expected
        && expression.rest.is_none()
        && expression
            .fields
            .iter()
            .all(|field| candidate_literal_expression(&field.expr))
}

fn candidate_literal_expression(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Lit(_) | syn::Expr::Path(_) => true,
        syn::Expr::Call(call) => {
            matches!(&*call.func, syn::Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "from" | "new" | "with_colors" | "Some")))
                && call.args.iter().all(candidate_literal_expression)
        }
        syn::Expr::Array(array) => array.elems.iter().all(candidate_literal_expression),
        syn::Expr::Macro(mac) if mac.mac.path.is_ident("vec") => syn::parse::Parser::parse2(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
            mac.mac.tokens.clone(),
        )
        .map(|items| items.iter().all(candidate_literal_expression))
        .unwrap_or(false),
        syn::Expr::Reference(reference) => {
            reference.mutability.is_none() && candidate_literal_expression(&reference.expr)
        }
        syn::Expr::Struct(structure) => candidate_struct_has_literal_shape_any_path(structure),
        _ => false,
    }
}

fn candidate_struct_has_literal_shape_any_path(expression: &syn::ExprStruct) -> bool {
    expression.rest.is_none()
        && expression
            .fields
            .iter()
            .all(|field| candidate_literal_expression(&field.expr))
}
