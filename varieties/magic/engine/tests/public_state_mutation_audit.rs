//! Mutation tests for the public fixture surface.
//!
//! `Game` intentionally exposes compact fixture state, so this suite corrupts
//! every public shape reachable without `unsafe` and requires the invariant
//! audit to reject it before a runner mistakes it for an engine transition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, ContinuousEffect, Duration, Effect, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, StackObject, Step, Target, TargetRequirement, Zone,
};

const BODY: &str = "MUTATION-BODY";
const BOLT: &str = "MUTATION-BOLT";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: BODY,
            name: BODY,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: BOLT,
            name: BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["deal-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Player,
            }],
        },
    ]
}

fn game(players: usize) -> Game {
    Game::new(definitions(), players).expect("fixture game initializes")
}

fn assert_rejected(game: &Game, label: &str) {
    assert!(
        game.validate_invariants().is_err(),
        "invariant audit accepted corrupted state: {label}"
    );
}

#[test]
fn mutation_audit_rejects_zone_and_stack_corruption() {
    let first = PlayerId(0);
    let second = PlayerId(1);

    let mut duplicate_zone = game(2);
    let card = duplicate_zone
        .add_card(first, BODY, Zone::Hand)
        .expect("known card enters hand");
    duplicate_zone.players[first.0].graveyard.push(card);
    assert_rejected(&duplicate_zone, "an object appears in two zones");

    let mut wrong_owner_zone = game(2);
    let card = wrong_owner_zone
        .add_card(first, BODY, Zone::Hand)
        .expect("known card enters hand");
    wrong_owner_zone.players[first.0].hand.clear();
    wrong_owner_zone.players[second.0].hand.push(card);
    assert_rejected(&wrong_owner_zone, "a card is in another player's hand");

    let mut missing_location = game(2);
    let card = missing_location
        .add_card(first, BODY, Zone::Hand)
        .expect("known card enters hand");
    missing_location.players[first.0].hand.clear();
    assert_rejected(
        &missing_location,
        "an allocated object has no zone or stack location",
    );
    assert_eq!(
        missing_location
            .object(card)
            .expect("object remains allocated")
            .id,
        card
    );

    let mut zone_and_stack = game(2);
    let card = zone_and_stack
        .add_card(first, BOLT, Zone::Hand)
        .expect("known instant enters hand");
    zone_and_stack.stack.push(StackObject {
        card,
        controller: first,
        ability_id: None,
        targets: vec![Target::Player(second)],
        effects: vec![Effect::DealDamage {
            amount: 1,
            target: TargetRequirement::Player,
        }],
        mana_spent: None,
    });
    assert_rejected(&zone_and_stack, "a stack card remains in a player zone");

    let mut fabricated_stack_target = game(2);
    let card = fabricated_stack_target
        .add_card(first, BOLT, Zone::Hand)
        .expect("known instant enters hand");
    fabricated_stack_target.players[first.0].hand.clear();
    fabricated_stack_target.stack.push(StackObject {
        card,
        controller: first,
        ability_id: None,
        targets: vec![Target::Player(second), Target::Player(first)],
        effects: vec![Effect::DealDamage {
            amount: 1,
            target: TargetRequirement::Player,
        }],
        mana_spent: None,
    });
    assert_rejected(
        &fabricated_stack_target,
        "stack object has extra fabricated targets",
    );

    let mut unknown_stack_card = game(2);
    unknown_stack_card.stack.push(StackObject {
        card: ObjectId(999),
        controller: first,
        ability_id: None,
        targets: vec![],
        effects: vec![],
        mana_spent: None,
    });
    assert_rejected(
        &unknown_stack_card,
        "stack object names an unallocated card",
    );
}

#[test]
fn mutation_audit_rejects_seat_priority_and_turn_corruption() {
    let first = PlayerId(0);
    let second = PlayerId(1);

    let mut invalid_priority = game(2);
    invalid_priority.priority = PlayerId(99);
    assert_rejected(&invalid_priority, "priority points beyond seated players");

    let mut invalid_active = game(2);
    invalid_active.active_player = PlayerId(99);
    assert_rejected(
        &invalid_active,
        "active player points beyond seated players",
    );

    let mut reassigned_seat = game(2);
    reassigned_seat.players[second.0].id = first;
    assert_rejected(&reassigned_seat, "seat vector and player ids disagree");

    let mut added_seat = game(2);
    let mut copied_player = added_seat.players[second.0].clone();
    copied_player.id = PlayerId(2);
    added_seat.players.push(copied_player);
    assert_rejected(&added_seat, "a fixture adds a seat after game creation");

    let mut lost_priority = game(3);
    lost_priority.players[first.0].lost = true;
    lost_priority.priority = first;
    lost_priority.active_player = second;
    assert_rejected(
        &lost_priority,
        "a continuing game gives priority to an eliminated seat",
    );

    let mut lost_active = game(3);
    lost_active.players[first.0].lost = true;
    lost_active.priority = second;
    assert_rejected(
        &lost_active,
        "a continuing game gives the turn to an eliminated seat",
    );

    let mut zero_turn = game(2);
    zero_turn.turn = 0;
    assert_rejected(&zero_turn, "turn number is zero");

    let mut extra_land = game(2);
    extra_land.players[first.0].lands_played = 2;
    assert_rejected(&extra_land, "a player has exceeded the land-play limit");
}

#[test]
fn mutation_audit_rejects_effect_and_step_marker_corruption() {
    let first = PlayerId(0);

    let mut zero_timestamp = game(2);
    let source = zero_timestamp
        .put_on_battlefield(first, BODY)
        .expect("source permanent enters battlefield");
    let target = zero_timestamp
        .put_on_battlefield(first, BODY)
        .expect("target permanent enters battlefield");
    zero_timestamp.continuous_effects.push(ContinuousEffect {
        source,
        target,
        change: ContinuousChange::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        },
        duration: Duration::Permanent,
        timestamp: 0,
    });
    assert_rejected(&zero_timestamp, "continuous-effect timestamp is zero");

    let mut stale_duration = game(2);
    let source = stale_duration
        .put_on_battlefield(first, BODY)
        .expect("source permanent enters battlefield");
    let target = stale_duration
        .put_on_battlefield(first, BODY)
        .expect("target permanent enters battlefield");
    stale_duration.continuous_effects.push(ContinuousEffect {
        source,
        target,
        change: ContinuousChange::AddColor(Color::Red),
        duration: Duration::EndOfTurn(stale_duration.turn + 1),
        timestamp: 1,
    });
    assert_rejected(
        &stale_duration,
        "an end-of-turn effect is assigned to another turn",
    );

    let mut automatic_step = game(2);
    automatic_step
        .begin_game()
        .expect("game begins at its upkeep priority boundary");
    automatic_step.step = Step::Cleanup;
    assert_rejected(
        &automatic_step,
        "cleanup cannot be a stable priority-bearing state",
    );

    let mut missing_combat = game(2);
    missing_combat.step = Step::DeclareAttackers;
    assert_rejected(
        &missing_combat,
        "combat declaration step lacks combat state",
    );
}

#[test]
fn mutation_audit_seals_event_log_and_permits_the_authorized_reset() {
    let first = PlayerId(0);
    let second = PlayerId(1);

    let mut appended_event = game(2);
    appended_event
        .event_log
        .push(GameEvent::PriorityPassed { player: first });
    assert_rejected(
        &appended_event,
        "a valid-looking event was appended externally",
    );

    let mut reordered_events = game(2);
    reordered_events
        .report_engine_weakness(first, "test.mutation", "first engine event")
        .expect("priority holder writes an engine event");
    reordered_events
        .report_engine_weakness(first, "test.mutation", "second engine event")
        .expect("priority holder writes another engine event");
    reordered_events.event_log.reverse();
    assert_rejected(
        &reordered_events,
        "event chronology was reordered externally",
    );

    let mut terminal_tail = game(2);
    terminal_tail
        .draw_card(second, None)
        .expect("empty-library draw produces terminal lifecycle receipts");
    assert!(matches!(
        terminal_tail.event_log.last(),
        Some(GameEvent::GameEnded { .. })
    ));
    *terminal_tail
        .event_log
        .last_mut()
        .expect("terminal event exists") = GameEvent::GameEnded {
        winner: Some(second),
    };
    assert_rejected(
        &terminal_tail,
        "terminal winner record was externally rewritten",
    );

    let mut authorized_reset = game(2);
    authorized_reset
        .report_engine_weakness(first, "test.mutation", "clear through API")
        .expect("priority holder writes an engine event");
    authorized_reset.clear_event_log();
    authorized_reset
        .validate_invariants()
        .expect("the explicit event-log reset updates the sealed lifecycle state");
}
