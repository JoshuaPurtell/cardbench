//! Red regression: a public mana action must produce mana or be rejected.

use cardbench_magic_engine::{CardDefinition, Color, Game, PlayerId, RulesError};

#[test]
fn zero_amount_public_mana_action_is_rejected_without_reopening_priority() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game =
        Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player fixture initializes");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.pass_priority(first)
        .expect("the first player opens the second player's response window");

    let mana_before = game
        .player(second)
        .expect("second player exists")
        .mana_pool
        .total();
    let events_before = game.event_log.clone();
    let result = game.add_mana_from_action(second, Color::Blue, 0);

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "a mana action must add positive mana"
            ))
        ),
        "zero mana was accepted as a priority action; mana before={mana_before}, mana after={}, priority={:?}, events before={events_before:?}, events after={:?}",
        game.player(second)
            .expect("second player exists")
            .mana_pool
            .total(),
        game.priority,
        game.event_log,
    );
    assert_eq!(
        game.player(second)
            .expect("second player exists")
            .mana_pool
            .total(),
        mana_before,
        "a rejected mana action must not change the mana pool"
    );
    assert_eq!(
        game.event_log, events_before,
        "rejection must be event-atomic"
    );
    assert_eq!(
        game.priority, second,
        "the existing response window remains intact"
    );
    game.validate_invariants()
        .expect("the rejected public action leaves a valid state");
}
