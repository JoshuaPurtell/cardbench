//! Red regression: a mana receipt must equal a real, atomic pool mutation.

use cardbench_magic_engine::{CardDefinition, Color, Game, PlayerId, RulesError};

#[test]
fn overflowing_public_mana_action_is_rejected_without_a_phantom_mana_receipt() {
    let player = PlayerId(0);
    let mut game =
        Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player fixture initializes");
    game.begin_game().expect("fixture reaches upkeep priority");
    game.add_mana_from_action(player, Color::Red, u8::MAX)
        .expect("the bounded pool accepts its maximum representable amount");
    game.clear_event_log();

    let pool_before = game
        .player(player)
        .expect("player exists")
        .mana_pool
        .amount(Color::Red);
    let events_before = game.event_log.clone();
    let result = game.add_mana_from_action(player, Color::Red, 1);

    assert!(
        matches!(
            result,
            Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana"
            ))
        ),
        "a saturated pool accepted a phantom mana action; pool before={pool_before}, pool after={}, events={:?}",
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Red),
        game.event_log,
    );
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::Red),
        pool_before,
        "a rejected capacity overflow must preserve the pool"
    );
    assert_eq!(
        game.event_log, events_before,
        "a rejected capacity overflow must not write a false ManaAdded receipt"
    );
    game.validate_invariants()
        .expect("the rejected action leaves a valid state");
}
