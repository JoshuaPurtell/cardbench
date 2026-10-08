//! Full-game RAV Goblin Fire Fiend haste, must-block, and pump trace.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, HybridManaSymbol, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{ObjectRef, ViewerScope};
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GOBLIN-FIRE-FIEND";
const BLOCKER_ID: &str = "RAV-LURKING-INFORMANT";
const SUPPORT_ID: &str = "RAV-GOBLIN-SPELUNKERS";
const ABILITY_ID: &str = "pump-plus-one-power";
const MAX_TRACE_SEED: u64 = 256;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/goblin_fire_fiend.rs"]
mod candidate_goblin_fire_fiend;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Goblin Fire Fiend PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.fiend != replay.fiend
        || captured.blocker != replay.blocker
        || captured.cast_payment != replay.cast_payment
        || captured.activation_payment != replay.activation_payment
        || captured.pump_resolution_characteristics != replay.pump_resolution_characteristics
    {
        return Err("Goblin Fire Fiend shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "haste_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveHaste)?,
        "must_block_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveMustBlock)?,
        "zero_power_activation_detected_in_game": mutation_detected(seed, Mutation::ZeroPump)?,
        "activation_color_mutation_detected_in_game": mutation_detected(seed, Mutation::BlueActivation)?,
        "cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongBasePower)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "haste_removal_detected_in_game",
        "must_block_removal_detected_in_game",
        "zero_power_activation_detected_in_game",
        "activation_color_mutation_detected_in_game",
        "cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Goblin Fire Fiend mutation escaped the trace oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 127, "definition_id": CARD_ID, "name": "Goblin Fire Fiend"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "302.6 Summoning sickness",
                "508.1 Declare attackers",
                "509.1-509.2 Declare blockers and satisfy blocking requirements",
                "510.1-510.2 Combat damage assignment and resolution",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities and costs",
                "608.2 Resolving an activated ability",
                "702.10 Haste"
            ],
            "source_contract": "RAV Goblin Fire Fiend #127, {3}{R} 1/1 red creature, Haste, must be blocked if able, and {R}: +1/+0 until end of turn",
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
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    RemoveHaste,
    RemoveMustBlock,
    ZeroPump,
    BlueActivation,
    WrongCastCost,
    WrongBasePower,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_goblin_fire_fiend::definition();
    let mut ability = candidate_goblin_fire_fiend::pump_plus_one_power_ability();
    match mutation {
        Mutation::None => {}
        Mutation::RemoveHaste => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Haste),
        Mutation::RemoveMustBlock => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::MustBeBlockedIfAble),
        Mutation::ZeroPump => {
            ability.effects = vec![Effect::ModifySourcePtUntilEndOfTurn {
                power: 0,
                toughness: 0,
            }];
        }
        Mutation::BlueActivation => ability.mana_cost = ManaCost::with_colors(0, [Color::Blue]),
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(4, [Color::Red]);
        }
        Mutation::WrongBasePower => definition.power = Some(2),
    }
    (definition, ability)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|registered| registered.id == CARD_ID)
        .ok_or("Goblin Fire Fiend definition is missing from the RAV registry")?;
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Goblin Fire Fiend activated ability is missing from the RAV registry")?;
    if registered_definition != definition || registered_ability.ability != ability {
        return Err(
            "Goblin Fire Fiend definition or ability is not delegated to its candidate module"
                .to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != "Goblin Fire Fiend"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || definition.keywords != [Keyword::Haste, Keyword::MustBeBlockedIfAble]
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
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
                power: 1,
                toughness: 0,
            }]
    {
        return Err(
            "Goblin Fire Fiend identity or feature mechanics differ from the RAV contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/goblin_fire_fiend.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Goblin Fire Fiend candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Goblin Fire Fiend module is not valid Rust source".to_owned())?;
    let function_names: BTreeSet<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let required = [
        "id:\"RAV-GOBLIN-FIRE-FIEND\"",
        "name:\"GoblinFireFiend\"",
        "mana_cost:ManaCost::with_colors(3,[Color::Red])",
        "power:Some(1)",
        "toughness:Some(1)",
        "Keyword::Haste",
        "Keyword::MustBeBlockedIfAble",
        "id:\"pump-plus-one-power\"",
        "Effect::ModifySourcePtUntilEndOfTurn{power:1,toughness:0",
    ];
    if function_names
        != BTreeSet::from([
            "definition".to_owned(),
            "pump_plus_one_power_ability".to_owned(),
        ])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err("Goblin Fire Fiend module is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn find_trace() -> Result<(u64, Captured), String> {
    let requested = std::env::var("CARDBENCH_GOBLIN_FIRE_FIEND_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    if let Some(seed) = requested {
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no Goblin Fire Fiend full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    fiend: Option<ObjectId>,
    blocker: Option<ObjectId>,
    support_damage: Option<(ObjectId, i32)>,
    cast_payment: Vec<Color>,
    activation_payment: Option<ManaCost>,
    cast_cost: ManaCost,
    ability: ActivatedAbility,
    fiend_zone: Option<Zone>,
    blocker_zone: Option<Zone>,
    pump_resolution_characteristics: Option<(Option<i32>, Option<i32>)>,
    life: [i64; 2],
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let (mut game, _definitions) = new_game(definition, ability.clone())?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SUPPORT_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if [p0.mainboard.as_slice(), p1.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| entry.count as usize)
                .sum::<usize>()
                != 60
        })
    {
        return Err("Goblin Fire Fiend decks must be shuffled legal 60-card lists".to_owned());
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
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GoblinFireFiendPolicy::new(
            PlayerId(0),
            &cast_cost,
            &ability.mana_cost,
            ability.id,
        )),
        Box::new(GoblinFireFiendPolicy::new(
            PlayerId(1),
            &ManaCost::with_hybrid(
                1,
                [],
                [HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                }],
            ),
            &ManaCost::new(0),
            "",
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut pump_resolution_characteristics = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Goblin Fire Fiend match exceeded its bounded move count".to_owned());
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
        let previous_event_count = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Goblin Fire Fiend policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        if pump_resolution_characteristics.is_none() {
            for event in &game.event_log[previous_event_count..] {
                if let GameEvent::AbilityResolved {
                    source,
                    ability: ABILITY_ID,
                    ..
                } = event
                {
                    if game
                        .card_definition(*source)
                        .is_ok_and(|card| card.id == CARD_ID)
                    {
                        let characteristics = game
                            .characteristics(*source)
                            .map_err(|error| error.to_string())?;
                        pump_resolution_characteristics =
                            Some((characteristics.power, characteristics.toughness));
                        break;
                    }
                }
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("Goblin Fire Fiend invariant failed after move: {error}"))?;
    }
    let events = game.event_log.clone();
    let fiend = events.iter().find_map(|event| match event {
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
    let blocker = fiend.and_then(|fiend| {
        events.iter().find_map(|event| match event {
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments
                .iter()
                .find_map(|(attacker, blocker)| (*attacker == fiend).then_some(*blocker)),
            _ => None,
        })
    });
    let cast_payment = fiend
        .and_then(|fiend| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == fiend => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let activation_payment = fiend.and_then(|fiend| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: ABILITY_ID,
                mana_cost,
            } if *source == fiend => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let support_damage = events.iter().find_map(|event| match event {
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount,
        } if Some(*source) != fiend
            && game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == SUPPORT_ID) =>
        {
            Some((*source, *amount))
        }
        _ => None,
    });
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let blocker_zone = blocker.and_then(|object| {
        events
            .iter()
            .any(|event| {
                matches!(
                    event,
                    GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == object
                )
            })
            .then_some(Zone::Graveyard)
    });
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        fiend,
        blocker,
        support_damage,
        cast_payment,
        activation_payment,
        cast_cost,
        ability,
        fiend_zone: fiend.and_then(|object| game.zone_of(object)),
        blocker_zone,
        pump_resolution_characteristics,
        life,
    })
}

fn new_game(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<(Game, Vec<CardDefinition>), String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Goblin Fire Fiend definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Goblin Fire Fiend ability binding missing from registry")?;
    binding.ability = ability;
    let game = Game::new_with_all_bindings(
        definitions.clone(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    Ok((game, definitions))
}

struct GoblinFireFiendPolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    activation_cost: ManaCost,
    ability_id: &'static str,
    pump_offered_turn: Option<u32>,
}

impl GoblinFireFiendPolicy {
    fn new(
        player: PlayerId,
        cast_cost: &ManaCost,
        activation_cost: &ManaCost,
        ability_id: &'static str,
    ) -> Self {
        Self {
            player,
            cast_cost: cast_cost.clone(),
            activation_cost: activation_cost.clone(),
            ability_id,
            pump_offered_turn: None,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let land = view.hand.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && (self.player == PlayerId(0) || card.definition == Some("RAV-ISLAND"))
        })?;
        Some(PolicyAction::PlayLand { card: land.id })
    }

    fn tap_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment_selection(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let count = available.get_mut(color)?;
            if *count == 0 {
                return None;
            }
            *count -= 1;
        }
        let mut hybrid = Vec::new();
        for symbol in &cost.hybrid {
            let color = if available.get(&symbol.first).copied().unwrap_or(0) > 0 {
                symbol.first
            } else if available.get(&symbol.second).copied().unwrap_or(0) > 0 {
                symbol.second
            } else {
                return None;
            };
            *available.get_mut(&color)? -= 1;
            hybrid.push(color);
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, count)| **count > 0)
                .max_by_key(|(_, count)| **count)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection { generic, hybrid })
    }

    fn cast_action(&self, view: &GameView, card_id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = self.payment_selection(view, cost) {
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let color = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                cost.hybrid.iter().find_map(|symbol| {
                    let first = view.mana_pool.amount(symbol.first);
                    let second = view.mana_pool.amount(symbol.second);
                    if first == 0 && second == 0 {
                        Some(symbol.first)
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                let total_required =
                    u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
                if u16::from(view.mana_pool.total()) >= total_required {
                    return None;
                }
                view.own_battlefield.iter().find_map(|land| {
                    (!land.tapped && land.card_types.contains(&CardType::Land))
                        .then(|| {
                            Color::ALL
                                .into_iter()
                                .find(|color| land.mana_colors.contains(color))
                        })
                        .flatten()
                })
            })?;
        self.tap_land(view, color)
    }

    fn activate_pump(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.pump_offered_turn == Some(view.turn)
            || view.active_player != self.player
            || view.step != Step::DeclareBlockers
            || !view.blockers_declared
            || view.stack_depth != 0
        {
            return None;
        }
        let blocked_fiend = view
            .combat_attackers
            .iter()
            .find(|attacker| attacker.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::Red) == 0 {
            let color = self
                .activation_cost
                .colored
                .first()
                .copied()
                .unwrap_or(Color::Red);
            return self.tap_land(view, color);
        }
        self.pump_offered_turn = Some(view.turn);
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: blocked_fiend.id,
                ability_id: self.ability_id,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }

    fn declare_blocks(&self, view: &GameView) -> PolicyAction {
        let required: Vec<_> = view
            .combat_attackers
            .iter()
            .filter(|attacker| {
                attacker.definition == Some(CARD_ID)
                    && attacker.keywords.contains(&Keyword::MustBeBlockedIfAble)
            })
            .collect();
        let blockers: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.can_block)
            .collect();
        let assignments = required
            .into_iter()
            .zip(blockers)
            .map(|(attacker, blocker)| cardbench_magic_engine::CombatBlock {
                attacker: attacker.id,
                blocker: blocker.id,
            })
            .collect();
        PolicyAction::DeclareBlockers { assignments }
    }
}

impl CodePolicy for GoblinFireFiendPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.goblin-fire-fiend-pump.v1"
        } else {
            "rav.lurking-informant-required-block.v1"
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
                    Vec::new()
                },
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return self.declare_blocks(view);
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.activate_pump(view) {
            return action;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(1) {
            let cost = ManaCost::with_hybrid(
                1,
                [],
                [HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                }],
            );
            if let Some(action) = self.cast_action(view, BLOCKER_ID, &cost) {
                return action;
            }
            return PolicyAction::PassPriority;
        }
        let total_available = u16::from(view.mana_pool.total())
            + u16::try_from(
                view.own_battlefield
                    .iter()
                    .filter(|card| {
                        !card.tapped
                            && card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(&Color::Red)
                    })
                    .count(),
            )
            .unwrap_or(u16::MAX);
        let cost_total = u16::from(self.cast_cost.generic)
            + self.cast_cost.colored.len() as u16
            + self.cast_cost.hybrid.len() as u16;
        if total_available >= cost_total + 1 {
            if let Some(action) = self.cast_action(view, CARD_ID, &self.cast_cost.clone()) {
                return action;
            }
        }
        if view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID) && card.summoning_sick && card.can_attack)
        {
            return PolicyAction::PassPriority;
        }
        let spelunkers_cost = ManaCost::with_colors(2, [Color::Red]);
        if let Some(action) = self.cast_action(view, SUPPORT_ID, &spelunkers_cost) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Goblin Fire Fiend match did not reach a clean seat-zero win".to_owned());
    }
    let fiend = captured
        .fiend
        .ok_or("seat zero did not cast Goblin Fire Fiend")?;
    let blocker = captured.blocker.ok_or_else(|| {
        let diagnostic: Vec<_> = captured.events.iter().filter(|event| matches!(event,
            GameEvent::SpellCast { .. }
                | GameEvent::CardMoved { to: Zone::Battlefield, .. }
                | GameEvent::AttackersDeclared { .. }
                | GameEvent::BlockersDeclared { .. }
        )).map(|event| format!("{event:?}")).collect();
        format!("a live blocker was not declared against Goblin Fire Fiend; combat diagnostic: {diagnostic:?}")
    })?;
    if captured.cast_cost != ManaCost::with_colors(3, [Color::Red])
        || captured.cast_payment.len() != 4
        || captured
            .cast_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 4
    {
        return Err(format!(
            "Goblin Fire Fiend exact {{3}}{{R}} spell payment was not observed: {:?}",
            captured.cast_payment
        ));
    }
    if captured.ability.id != ABILITY_ID
        || captured.ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || captured.ability.tap_cost
        || captured.ability.sorcery_speed
        || captured.ability.sacrifice_source
        || captured.ability.targets.len() != 0
        || captured.ability.effects
            != [Effect::ModifySourcePtUntilEndOfTurn {
                power: 1,
                toughness: 0,
            }]
        || captured.activation_payment.as_ref() != Some(&ManaCost::with_colors(0, [Color::Red]))
    {
        return Err(format!(
            "Goblin Fire Fiend's red plus-one-power activation was not paid and resolved as printed: ability={:?}, payment={:?}, abilities={:?}",
            captured.ability,
            captured.activation_payment,
            captured
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::AbilityManaPaid { .. }
                        | GameEvent::AbilityActivated { .. }
                        | GameEvent::AbilityResolved { .. }
                ))
                .map(|event| format!("{event:?}"))
                .collect::<Vec<_>>()
        ));
    }
    if captured.pump_resolution_characteristics != Some((Some(2), Some(1))) {
        return Err(format!(
            "the activated ability did not give its source live 2/1 characteristics: {:?}",
            captured.pump_resolution_characteristics
        ));
    }
    if captured.blocker_zone != Some(Zone::Graveyard)
        || captured.fiend_zone != Some(Zone::Graveyard)
    {
        let diagnostic: Vec<_> = captured
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::BlockersDeclared { .. }
                        | GameEvent::AbilityManaPaid { .. }
                        | GameEvent::AbilityActivated { .. }
                        | GameEvent::AbilityResolved { .. }
                        | GameEvent::DamageDealtToPermanent { .. }
                        | GameEvent::StateBasedAction { .. }
                        | GameEvent::CardMoved {
                            to: Zone::Graveyard,
                            ..
                        }
                )
            })
            .map(|event| format!("{event:?}"))
            .collect();
        return Err(format!(
            "the pumped combat did not destroy both the 1/2 blocker and 1/1 attacker: fiend={:?}, blocker={:?}, trace={diagnostic:?}",
            captured.fiend_zone, captured.blocker_zone
        ));
    }
    let events = &captured.events;
    let cast_paid = find_event(events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == fiend && colors.len() == 4 && colors.iter().all(|color| *color == Color::Red)))
        .ok_or("four-red spell payment receipt is missing")?;
    let cast = find_event_after(events, cast_paid, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == fiend)
    })
    .ok_or("Goblin Fire Fiend SpellCast receipt is missing")?;
    let resolved = find_event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == fiend),
    )
    .ok_or("Goblin Fire Fiend SpellResolved receipt is missing")?;
    let entered = find_event_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == fiend))
        .ok_or("Goblin Fire Fiend did not enter the battlefield")?;
    let attack = find_event_after(events, entered, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&fiend)))
        .ok_or("Haste-enabled Goblin Fire Fiend did not attack")?;
    let block = find_event_after(events, attack, |event| {
        matches!(event,
        GameEvent::BlockersDeclared { player: PlayerId(1), assignments }
            if assignments.contains(&(fiend, blocker)))
    })
    .ok_or("the live Lurking Informant did not satisfy Fire Fiend's blocking requirement")?;
    let paid = find_event_after(events, block, |event| {
        matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
            if *source == fiend && mana_cost == &ManaCost::with_colors(0, [Color::Red]))
    })
    .ok_or("red mana payment for the pump ability is missing")?;
    let activated = find_event_after(events, paid, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. }
            if *source == fiend))
        .ok_or("Goblin Fire Fiend activation receipt is missing")?;
    let ability_resolved = find_event_after(events, activated, |event| {
        matches!(event,
        GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == fiend)
    })
    .ok_or("the pump ability did not resolve before combat damage")?;
    let damage_to_blocker = find_event_after(events, ability_resolved, |event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
            if *source == fiend && *permanent == blocker)
    })
    .ok_or("the pumped 2 power did not deal lethal combat damage to the blocker")?;
    let damage_to_fiend = find_event_after(events, damage_to_blocker, |event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
            if *source == blocker && *permanent == fiend)
    })
    .ok_or("the blocking Informant did not deal its 1 combat damage back to the Fire Fiend")?;
    let lethal_blocker = find_event_after(events, damage_to_blocker, |event| matches!(event,
        GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == blocker))
        .ok_or("lethal combat damage did not cause the blocker state-based action")?;
    let blocker_died = find_event_after(events, lethal_blocker, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == blocker)
    })
    .ok_or("blocked Lurking Informant did not move to its graveyard")?;
    let lethal_fiend = find_event_after(events, damage_to_fiend, |event| matches!(event,
        GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == fiend))
        .ok_or("the Fire Fiend's lethal combat damage did not cause its state-based action")?;
    let fiend_died = find_event_after(events, lethal_fiend, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == fiend)
    })
    .ok_or("the Fire Fiend did not move to its graveyard after combat")?;
    let later_attack = find_event_after(events, fiend_died.max(blocker_died), |event| {
        matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if !attackers.is_empty())
    })
    .ok_or("a later seat-zero combat attack is missing")?;
    let later_damage = find_event_after(events, later_attack, |event| {
        matches!(event,
        GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } if *amount > 0)
    })
    .ok_or("a later combat did not damage the opponent")?;
    let ended = find_event_after(events, later_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat zero's terminal win is missing")?;
    if !(cast_paid < cast
        && cast < resolved
        && resolved < entered
        && entered < attack
        && attack < block
        && block < paid
        && paid < activated
        && activated < ability_resolved
        && ability_resolved < damage_to_blocker
        && damage_to_blocker < damage_to_fiend
        && damage_to_blocker < lethal_blocker
        && lethal_blocker < blocker_died
        && damage_to_fiend < lethal_fiend
        && lethal_fiend < fiend_died
        && blocker_died < later_attack
        && fiend_died < later_attack
        && later_attack < later_damage
        && later_damage < ended)
    {
        return Err("Goblin Fire Fiend cost, Haste attack, blocking, pump, and combat events are out of order".to_owned());
    }
    let projected_all = project_events(events);
    let cast_turn = projected_all
        .iter()
        .find(|event| {
            event.kind == "SpellCast" && event.facts.objects.contains(&ObjectRef(fiend.0))
        })
        .map(|event| event.turn)
        .ok_or("projected Goblin Fire Fiend cast turn is missing")?;
    let attack_turn = projected_all
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared" && event.facts.objects.contains(&ObjectRef(fiend.0))
        })
        .map(|event| event.turn)
        .ok_or("projected Haste attack turn is missing")?;
    if cast_turn != attack_turn {
        return Err("Goblin Fire Fiend did not attack in the turn it entered".to_owned());
    }
    let selected = BTreeSet::from([
        cast_paid,
        cast,
        resolved,
        entered,
        attack,
        block,
        paid,
        activated,
        ability_resolved,
        damage_to_blocker,
        damage_to_fiend,
        lethal_blocker,
        blocker_died,
        lethal_fiend,
        fiend_died,
        later_attack,
        later_damage,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = projected_all
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    let support = captured.support_damage;
    Ok(json!({
        "trace_id": "rav-goblin-fire-fiend-haste-must-block-pump-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_goblin_fire_fiend_mountain", "rav_lurking_informant_island"],
        "policies": ["rav.goblin-fire-fiend-pump.v1", "rav.lurking-informant-required-block.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "goblin_fire_fiend": {"object": fiend.0, "collector_number": 127, "mana_cost": "{3}{R}", "mana_paid": captured.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "base_power": 1, "base_toughness": 1, "haste_attack_same_turn": true, "zone_after_combat": "Graveyard"},
        "blocker": {"object": blocker.0, "definition": BLOCKER_ID, "power": 1, "toughness": 2, "damage": 2, "zone_after_damage": "Graveyard", "blocked_fiend": fiend.0},
        "activation": {"ability": ABILITY_ID, "source": fiend.0, "mana_cost": "{R}", "red_mana_paid": true, "power_after_resolution": 2, "toughness_after_resolution": 1, "effect_resolved_before_combat_damage": true},
        "combat": {"must_block_requirement_satisfied": true, "fiend_dealt_lethal_damage": true, "fiend_later_combat_damage": support.map(|(_, amount)| amount), "opponent_life_at_terminal": captured.life[1], "life_at_terminal": captured.life},
        "events": projected,
        "passed": true
    }))
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let status = if card.semantic_status.is_executable() {
        "executable_compatibility_slice"
    } else {
        "catalog_only"
    };
    let definition_id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":status,"trace_status":if definition_id==CARD_ID{"passed"}else{"not_qualified"},
        "qualification":if definition_id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
