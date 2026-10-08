//! Full-game diagnostic authority for RAV Lore Broker.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CatalogPolicyProfile, CodePolicy, RavCatalogPolicy};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LORE-BROKER";
const CARD_NAME: &str = "Lore Broker";
const ABILITY_ID: &str = "each-player-draws-then-discards";
const FINISHER_ID: &str = "RAV-GOLIATH-SPIDER";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/lore_broker.rs";
const MAX_TRACE_SEED: u64 = 96;

const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Lore Broker definition and activated ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
};
use std::collections::BTreeSet;

/// Printed blue 1/2 creature with an each-player draw-and-discard tap ability.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LORE-BROKER",
        name: "Lore Broker",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-activated-each-player-draws-one",
            "simultaneous-private-each-player-discard-after-draw",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Each player draws one card, then each chooses and discards one privately.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-LORE-BROKER",
        ability: ActivatedAbility {
            id: "each-player-draws-then-discards",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::EachPlayerDrawsThenDiscardsOneCard],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/lore_broker.rs"]
mod candidate_lore_broker;

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
            eprintln!("Lore Broker PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let side_effect_rejected = malicious_candidate_source_rejected()?;
    if !side_effect_rejected {
        return Err("Lore Broker compile-valid side-effect source was not rejected before candidate execution".into());
    }
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Lore Broker full-game event replay is not deterministic".into());
    }
    let mutations = json!({
        "wrong_body_detected_in_game": mutation_detected(seed, Mutation::WrongBody),
        "wrong_spell_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "missing_tap_cost_detected_in_game": mutation_detected(seed, Mutation::NoTapCost),
        "draw_discard_effect_removed_detected_in_game": mutation_detected(seed, Mutation::NoEffect),
        "draw_omitted_by_wrong_effect_detected_in_game": mutation_detected(seed, Mutation::DiscardOnly),
        "compile_valid_candidate_side_effect_rejected_before_execution": side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_body_detected_in_game",
        "wrong_spell_cost_detected_in_game",
        "missing_tap_cost_detected_in_game",
        "draw_discard_effect_removed_detected_in_game",
        "draw_omitted_by_wrong_effect_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Lore Broker semantic/source mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 57, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["117.1b and 117.3c priority", "301.1 and 302.1 permanent and creature characteristics", "602.1-602.2 activated abilities and costs", "608.2 ability resolution", "121.2-121.3 drawing cards", "400.2 public and hidden zones", "701.8 discarding cards", "508 and 510 combat"],
            "source_contract": "RAV Lore Broker #57 is a blue {1}{U} 1/2 creature. Its {T} ability makes each player draw a card, then discard a card from their own hand.",
            "subtype_limit": "The current CardDefinition schema has no creature subtype field; printed Human Rogue is documented but not scored as an engine characteristic.",
            "oracle_reconciliation": "Modern Oracle pronouns preserve the printed RAV each-player hand-discard operation.",
            "authority_cutoff": "2026-09-24",
            "public_identity_record": "https://www.mtg.wtf/card/rav/57/Lore-Broker",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "source_guard_attestation": {
            "candidate_source_ast_and_literals_validated_before_candidate_calls": true,
            "compile_valid_injected_filesystem_write_rejected_before_execution": side_effect_rejected,
            "candidate_calls_before_malicious_source_rejection": 0
        },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongBody,
    WrongCost,
    NoTapCost,
    NoEffect,
    DiscardOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    broker: Option<ObjectId>,
    broker_paid: Vec<Color>,
    broker_cost: ManaCost,
    broker_power: Option<i32>,
    broker_toughness: Option<i32>,
    activation_source: Option<ObjectId>,
    activation_count: usize,
    source_tapped_after_activation: bool,
    ability_resolved: bool,
    decision_seats: Vec<usize>,
    private_decisions: Vec<bool>,
    candidate_counts: Vec<usize>,
    choices_hidden_from_other_player: bool,
    candidates_owned_by_chooser: bool,
    both_players_alive_at_choices: bool,
    draws_before_first_choice: Vec<usize>,
    drawn_objects_in_hand_at_first_choice: bool,
    no_discard_before_both_choices: bool,
    discarded_seats: Vec<usize>,
    discard_owners_match: bool,
    spider: Option<ObjectId>,
    spider_attack_turns: Vec<u32>,
    spider_damage: i32,
    terminal: String,
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no deterministic Lore Broker reference seed passed in 0..{MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: FINISHER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 32,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    let deck_size = |deck: &DeckList| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
    };
    if deck_size(&p0) != 60 || deck_size(&p1) != 60 {
        return Err("Lore Broker authority decks must each contain 60 cards".into());
    }

    let mut definition = candidate_lore_broker::definition();
    let mut ability = candidate_lore_broker::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongBody => definition.power = Some(2),
        Mutation::WrongCost => definition.mana_cost = ManaCost::new(2),
        Mutation::NoTapCost => ability.ability.tap_cost = false,
        Mutation::NoEffect => ability.ability.effects.clear(),
        Mutation::DiscardOnly => ability.ability.effects = vec![Effect::DiscardOneCardEachPlayer],
    }
    let broker_cost = definition.mana_cost.clone();
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Lore Broker is absent from the RAV definition registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    *abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Lore Broker is absent from the RAV activated ability registry")? = ability;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut pilot = BrokerPolicy::new(PlayerId(0));
    pilot.broker_cost = Some(broker_cost.clone());
    let mut policies: [Box<dyn CodePolicy>; 2] =
        [Box::new(pilot), Box::new(BrokerPolicy::new(PlayerId(1)))];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        broker: None,
        broker_paid: vec![],
        broker_cost,
        broker_power: None,
        broker_toughness: None,
        activation_source: None,
        activation_count: 0,
        source_tapped_after_activation: false,
        ability_resolved: false,
        decision_seats: vec![],
        private_decisions: vec![],
        candidate_counts: vec![],
        choices_hidden_from_other_player: true,
        candidates_owned_by_chooser: true,
        both_players_alive_at_choices: true,
        draws_before_first_choice: vec![],
        drawn_objects_in_hand_at_first_choice: false,
        no_discard_before_both_choices: true,
        discarded_seats: vec![],
        discard_owners_match: true,
        spider: None,
        spider_attack_turns: vec![],
        spider_damage: 0,
        terminal: String::new(),
    };
    let max_turns = 40_u32;
    let max_moves = 900_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > max_turns || captured.accepted >= max_moves {
            return Err(format!(
                "Lore Broker game exceeded its bound at turn {} after {} moves",
                game.turn, captured.accepted
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        observe_private_choice(&game, &view, &mut captured)?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Lore Broker policy seat is missing")?;
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
        let activation_source = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some(activation.source)
            }
            _ => None,
        };
        let cursor = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Lore Broker policy move was rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Lore Broker invariant failed: {error}"))?;
        if let Some(source) = activation_source {
            captured.activation_source = Some(source);
            captured.source_tapped_after_activation =
                game.object(source).is_ok_and(|object| object.tapped);
        }
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    captured.broker = Some(*card);
                    captured.broker_paid = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    captured.broker = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    ..
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == FINISHER_ID) =>
                {
                    captured.spider = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == FINISHER_ID) =>
                {
                    captured.spider = Some(*card)
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    ability,
                    ..
                } if *ability == ABILITY_ID => {
                    captured.activation_source = Some(*source);
                    captured.activation_count += 1;
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if Some(*source) == captured.activation_source && *ability == ABILITY_ID => {
                    captured.ability_resolved = true
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if captured.activation_source.is_some() && !captured.ability_resolved => {
                    let activation_index = game.event_log.iter().position(|known| matches!(known,
                            GameEvent::AbilityActivated { source, ability, .. }
                                if Some(*source) == captured.activation_source && *ability == ABILITY_ID));
                    if activation_index.is_some_and(|index| game.event_log.len() > index + 1) {
                        let owner = game
                            .object(*card)
                            .map_err(|error| error.to_string())?
                            .owner
                            .0;
                        if !captured.draws_before_first_choice.contains(&owner) {
                            captured.draws_before_first_choice.push(owner);
                        }
                    }
                }
                GameEvent::CardDiscarded { player, card }
                    if captured.activation_source.is_some() =>
                {
                    captured.discarded_seats.push(player.0);
                    captured.discard_owners_match &= game
                        .object(*card)
                        .is_ok_and(|object| object.owner == *player);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if captured
                    .spider
                    .is_some_and(|spider| attackers.contains(&spider)) =>
                {
                    captured.spider_attack_turns.push(game.turn)
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if Some(*source) == captured.spider => captured.spider_damage += *amount,
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.rejected = attempted.saturating_sub(captured.accepted);
    if let Some(source) = captured.broker {
        let characteristics = game
            .characteristics(source)
            .map_err(|error| error.to_string())?;
        captured.broker_power = characteristics.power;
        captured.broker_toughness = characteristics.toughness;
    }
    captured.terminal = game.winner().map_or_else(
        || "NoWinner".to_owned(),
        |winner| format!("Winner({winner:?})"),
    );
    Ok(captured)
}

fn observe_private_choice(
    game: &Game,
    view: &GameView,
    captured: &mut Captured,
) -> Result<(), String> {
    let Some(decision) = view.pending_decision.as_ref() else {
        return Ok(());
    };
    if decision.kind != DecisionKind::TriggeredEffectObject {
        return Ok(());
    }
    let Some(choice) = view.triggered_ability_effect_object_choice.as_ref() else {
        return Ok(());
    };
    if Some(choice.source) != captured.activation_source || choice.ability != ABILITY_ID {
        return Ok(());
    }
    let chooser = view.player;
    let other = PlayerId(1_usize.saturating_sub(chooser.0));
    let other_view = game
        .view_for_player(other)
        .map_err(|error| error.to_string())?;
    captured.decision_seats.push(chooser.0);
    captured
        .private_decisions
        .push(decision.visibility == DecisionVisibility::Private);
    captured.candidate_counts.push(choice.candidates.len());
    captured.choices_hidden_from_other_player &= other_view.pending_decision.is_none()
        && other_view.triggered_ability_effect_object_choice.is_none();
    captured.candidates_owned_by_chooser &= !choice.candidates.is_empty()
        && choice.candidates.iter().all(|candidate| {
            game.object(candidate.id)
                .is_ok_and(|object| object.owner == chooser)
                && game.zone_of(candidate.id) == Some(Zone::Hand)
        });
    captured.both_players_alive_at_choices &=
        view.own_life > 0 && view.opponent_life.iter().any(|(_, life)| *life > 0);
    let activation_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(event,
        GameEvent::AbilityActivated { source, ability, .. }
            if *source == choice.source && *ability == ABILITY_ID)
        })
        .ok_or("Lore Broker private choice appeared without an activation receipt")?;
    let draws: Vec<_> = game
        .event_log
        .iter()
        .skip(activation_index + 1)
        .take_while(|event| {
            !matches!(
                event,
                GameEvent::DecisionOpened {
                    kind: DecisionKind::TriggeredEffectObject,
                    ..
                }
            )
        })
        .filter_map(|event| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } => Some(*card),
            _ => None,
        })
        .collect();
    let mut owners: Vec<_> = draws
        .iter()
        .filter_map(|card| game.object(*card).ok().map(|object| object.owner.0))
        .collect();
    owners.sort_unstable();
    if captured.decision_seats.len() == 1 {
        captured.drawn_objects_in_hand_at_first_choice = draws.len() == 2
            && owners == vec![0, 1]
            && draws
                .iter()
                .all(|card| game.zone_of(*card) == Some(Zone::Hand));
        captured.no_discard_before_both_choices = !game
            .event_log
            .iter()
            .skip(activation_index + 1)
            .any(|event| matches!(event, GameEvent::CardDiscarded { .. }));
        captured.draws_before_first_choice = owners;
    } else {
        captured.no_discard_before_both_choices &= !game
            .event_log
            .iter()
            .skip(activation_index + 1)
            .any(|event| matches!(event, GameEvent::CardDiscarded { .. }));
    }
    Ok(())
}

struct BrokerPolicy {
    player: PlayerId,
    delegate: RavCatalogPolicy,
    broker_cost: Option<ManaCost>,
    activation_offered: bool,
    spider_cast: bool,
}

impl BrokerPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            delegate: RavCatalogPolicy::new(player, CatalogPolicyProfile::Patient),
            broker_cost: None,
            activation_offered: false,
            spider_cast: false,
        }
    }

    fn land_to_play(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let want_island = !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some("RAV-ISLAND"));
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && (self.player != PlayerId(0)
                        || (want_island && card.definition == Some("RAV-ISLAND"))
                        || (!want_island && card.definition == Some("RAV-FOREST")))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn land_for_color(&self, view: &GameView, color: Option<Color>) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && color.map_or(true, |color| card.mana_colors.contains(&color))
        })?;
        let color = color
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().copied().next())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let colors = Color::MANA_ALL;
        let mut available: Vec<_> = colors
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for required in &cost.colored {
            let (_, amount) = available.iter_mut().find(|(color, _)| color == required)?;
            *amount = amount.checked_sub(1)?;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, amount) = available.iter_mut().find(|(_, amount)| *amount > 0)?;
            *amount -= 1;
            generic.push(*color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn mana_action(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing_color = cost.colored.iter().copied().find(|color| {
            let required = cost
                .colored
                .iter()
                .filter(|required| *required == color)
                .count();
            usize::from(view.mana_pool.amount(*color)) < required
        });
        Self::land_for_color(view, missing_color)
    }

    fn cast(card: ObjectId, payment: ManaPaymentSelection) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: payment,
        }
    }
}

impl CodePolicy for BrokerPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.lore-broker-pilot.v1"
        } else {
            "rav.lore-broker-opponent.v1"
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind == DecisionKind::TriggeredEffectObject
            && decision.visibility == DecisionVisibility::Private
            && view.player == self.player
        {
            let selected = decision
                .candidates
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
                .or_else(|| {
                    decision
                        .candidates
                        .iter()
                        .find(|card| card.definition != Some(FINISHER_ID))
                })
                .or_else(|| decision.candidates.first())?;
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(vec![selected.id]),
            });
        }
        self.delegate.propose_pending_decision(view)
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.delegate.propose_draw_replacement(view)
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
                        .filter(|card| card.definition == Some(FINISHER_ID) && card.can_attack)
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
        if view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.land_to_play(view) {
            return land;
        }
        if self.player == PlayerId(0) {
            let broker_on_battlefield = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !broker_on_battlefield {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    let cost = self
                        .broker_cost
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(|| candidate_lore_broker::definition().mana_cost);
                    if let Some(payment) = Self::payment(view, &cost) {
                        return Self::cast(card.id, payment);
                    }
                    return Self::mana_action(view, &cost).unwrap_or(PolicyAction::PassPriority);
                }
            }
            if !self.activation_offered && view.turn >= 3 {
                if let Some(source) = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
                }) {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility {
                        activation: cardbench_magic_engine::AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
            }
            if !self.spider_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(FINISHER_ID))
            {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(FINISHER_ID))
                {
                    let cost = ManaCost::with_colors(4, [Color::Green, Color::Green]);
                    if let Some(payment) = Self::payment(view, &cost) {
                        self.spider_cast = true;
                        return Self::cast(card.id, payment);
                    }
                    return Self::mana_action(view, &cost).unwrap_or(PolicyAction::PassPriority);
                }
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
        return Err("Lore Broker did not reach a clean seat-zero terminal win".into());
    }
    let broker = captured.broker.ok_or("Lore Broker was not cast")?;
    let source = captured
        .activation_source
        .ok_or("Lore Broker ability was not activated")?;
    let spider = captured
        .spider
        .ok_or("Goliath Spider finisher was not cast")?;
    if captured.broker_paid.len() != 2
        || !captured.broker_paid.contains(&Color::Blue)
        || captured.broker_cost != ManaCost::with_colors(1, [Color::Blue])
        || captured.broker_power != Some(1)
        || captured.broker_toughness != Some(2)
        || captured.activation_count != 1
        || source != broker
        || !captured.source_tapped_after_activation
        || !captured.ability_resolved
        || captured.decision_seats != [0, 1]
        || captured.private_decisions != [true, true]
        || captured.candidate_counts.len() != 2
        || captured.candidate_counts.iter().any(|count| *count == 0)
        || !captured.choices_hidden_from_other_player
        || !captured.candidates_owned_by_chooser
        || !captured.both_players_alive_at_choices
        || captured.draws_before_first_choice != [0, 1]
        || !captured.drawn_objects_in_hand_at_first_choice
        || !captured.no_discard_before_both_choices
        || captured.discarded_seats != [0, 1]
        || !captured.discard_owners_match
        || captured.spider_attack_turns.len() < 3
        || captured.spider_damage < 21
    {
        return Err(format!(
            "Lore Broker identity, privacy, draw/discard order, or full-game proof differs: {captured:?}"
        ));
    }
    let events = &captured.events;
    let activation = event_index(events, |event| {
        matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source: known, ability, .. }
            if *known == source && *ability == ABILITY_ID)
    })
    .ok_or("Lore Broker activation receipt missing")?;
    let first_choice = event_index_after(events, activation, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                visibility: DecisionVisibility::Private,
                min_selections: 1,
                max_selections: 1,
                ..
            }
        )
    })
    .ok_or("P0 private discard decision receipt missing")?;
    let draw_moves: Vec<_> = events
        .iter()
        .enumerate()
        .skip(activation + 1)
        .take_while(|(_, event)| {
            !matches!(
                event,
                GameEvent::DecisionOpened {
                    kind: DecisionKind::TriggeredEffectObject,
                    ..
                }
            )
        })
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }).then_some(index)
        })
        .collect();
    let second_choice = event_index_after(events, first_choice, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(1),
                kind: DecisionKind::TriggeredEffectObject,
                visibility: DecisionVisibility::Private,
                min_selections: 1,
                max_selections: 1,
                ..
            }
        )
    })
    .ok_or("P1 private discard decision receipt missing")?;
    let first_completed = event_index_after(events, first_choice, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("P0 private decision completion receipt missing")?;
    let second_completed = event_index_after(events, second_choice, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(1),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("P1 private decision completion receipt missing")?;
    let discards: Vec<_> = events
        .iter()
        .enumerate()
        .skip(second_completed + 1)
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::CardDiscarded { .. }).then_some(index)
        })
        .collect();
    let resolved = event_index_after(events, second_completed, |event| matches!(event,
        GameEvent::AbilityResolved { source: known, ability, .. } if *known == source && *ability == ABILITY_ID))
        .ok_or("Lore Broker resolution receipt missing")?;
    let spider_attack = event_index_after(events, resolved, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&spider)))
        .ok_or("Goliath Spider did not attack after Lore Broker resolved")?;
    let spider_damage = event_index_after(events, spider_attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source: known, player: PlayerId(1), amount } if *known == spider && *amount > 0))
        .ok_or("Goliath Spider damage receipt missing")?;
    let ended = event_index_after(events, spider_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero GameEnded receipt missing")?;
    let discard_owners: Vec<_> = events
        .iter()
        .skip(second_completed + 1)
        .filter_map(|event| match event {
            GameEvent::CardDiscarded { player, .. } => Some(player.0),
            _ => None,
        })
        .collect();
    if draw_moves.len() != 2
        || discards.len() != 2
        || discard_owners != [0, 1]
        || !(activation < draw_moves[0]
            && draw_moves[0] < draw_moves[1]
            && draw_moves[1] < first_choice
            && first_choice < first_completed
            && first_completed < second_choice
            && second_choice < second_completed
            && second_completed < discards[0]
            && discards[0] < discards[1]
            && discards[1] < resolved
            && resolved < spider_attack
            && spider_attack < spider_damage
            && spider_damage < ended)
    {
        return Err("Lore Broker event receipts do not show both draws before private choices and both discards after them".into());
    }
    Ok(json!({
        "id": "rav-lore-broker-each-player-private-draw-discard-full-game",
        "passed": true,
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "lore_broker": {"object": broker.0, "mana_cost": "{1}{U}", "paid_colors": captured.broker_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": captured.broker_power, "toughness": captured.broker_toughness, "subtype_schema_limit": "Human Rogue not represented by CardDefinition"},
        "activation": {"source": source.0, "ability_id": ABILITY_ID, "count": captured.activation_count, "tap_cost_paid_by_state": captured.source_tapped_after_activation, "resolved": captured.ability_resolved},
        "draw_discard": {"draws_by_seat_before_first_choice": captured.draws_before_first_choice, "drawn_cards_still_in_hand_at_first_choice": captured.drawn_objects_in_hand_at_first_choice, "private_choice_seats": captured.decision_seats, "candidate_counts_by_chooser": captured.candidate_counts, "private_to_each_chooser": captured.private_decisions, "hidden_from_other_player": captured.choices_hidden_from_other_player, "candidate_ownership_matches_chooser": captured.candidates_owned_by_chooser, "both_players_alive_during_choices": captured.both_players_alive_at_choices, "no_discard_before_both_choices": captured.no_discard_before_both_choices, "discarded_by_seat": captured.discarded_seats, "discard_owners_match": captured.discard_owners_match},
        "combat": {"finisher": "Goliath Spider", "object": spider.0, "attack_turns": captured.spider_attack_turns, "damage_to_opponent": captured.spider_damage},
        "ordered_event_sequence": [activation, draw_moves[0], draw_moves[1], first_choice, first_completed, second_choice, second_completed, discards[0], discards[1], resolved, spider_attack, spider_damage, ended],
        "events": redact_private_hand_objects(events)
    }))
}

fn redact_private_hand_objects(events: &[GameEvent]) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .map(|mut event| {
            if event.kind == "CardMoved" && event.facts.zone.as_deref() == Some("Hand") {
                event.facts.objects.clear();
                event.canonical = "CardMoved { card: <private-hand-object>, to: Hand }".to_owned();
            }
            event
        })
        .collect()
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Lore Broker candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Lore Broker source is not valid Rust".to_owned())?;
    if source != EXPECTED_CANDIDATE_SOURCE
        || normalize_source(source) != normalize_source(EXPECTED_CANDIDATE_SOURCE)
        || file.items.len() != 4
        || !file.attrs.iter().all(|attr| attr.path().is_ident("doc"))
    {
        return Err("Lore Broker candidate source AST or literal contract differs".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(item) => {
                let (name, output) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "activated_ability" => ("activated_ability", "ActivatedAbilityBinding"),
                    _ => return Err("Lore Broker candidate source AST or literal contract differs".into()),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = item.block.stmts.as_slice() else {
                    return Err("Lore Broker candidate functions must return a direct literal".into());
                };
                if !matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !item.sig.inputs.is_empty() || !item.sig.generics.params.is_empty()
                    || item.sig.generics.where_clause.is_some() || !return_type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output) || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err("Lore Broker candidate source AST or literal contract differs".into());
                }
            }
            _ => return Err("Lore Broker candidate source may contain only imports and direct literal functions".into()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()]) {
        return Err("Lore Broker candidate source is missing a required literal".into());
    }
    Ok(())
}

fn normalize_source(source: &str) -> String {
    source.split_whitespace().collect::<String>()
}

fn return_type_is(output: &ReturnType, expected: &str) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn malicious_candidate_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let marker = "pub(crate) fn definition() -> CardDefinition {\n    CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-lore-broker-candidate-executed\", b\"executed\");\n    CardDefinition {";
    let malicious = source.replacen(marker, injected, 1);
    let syntax_valid = malicious != source && syn::parse_file(&malicious).is_ok();
    if !syntax_valid {
        return Err(
            "could not construct the compile-valid Lore Broker side-effect candidate".into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_lore_broker::definition();
    let ability = candidate_lore_broker::activated_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|known| known.id == CARD_ID)
        .ok_or("Lore Broker is missing from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(2)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-activated-each-player-draws-one",
                "simultaneous-private-each-player-discard-after-draw",
            ]
        || ability.card_definition != CARD_ID
        || ability.ability.id != ABILITY_ID
        || ability.ability.mana_cost != ManaCost::new(0)
        || !ability.ability.tap_cost
        || ability.ability.sorcery_speed
        || ability.ability.additional_tap_creatures != 0
        || ability.ability.sacrifice_source
        || ability.ability.sacrifice_creatures != 0
        || ability.ability.sacrifice_lands != 0
        || ability.ability.discard_cards != 0
        || !ability.ability.targets.is_empty()
        || ability.ability.effects != [Effect::EachPlayerDrawsThenDiscardsOneCard]
        || !rav_activated_ability_bindings()
            .iter()
            .any(|known| known == &ability)
    {
        return Err("Lore Broker candidate definition or activated ability differs from its RAV source contract".into());
    }
    Ok(())
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
            "RAV catalog denominator differs from the 291-name / 306-printing source contract"
                .into(),
        );
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 57;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},
        "trace_status":if passed{"passed"}else{"not_qualified"},
        "qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}
