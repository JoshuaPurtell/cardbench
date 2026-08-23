//! RED: an external live-state edit must not be laundered through the
//! receipt-reset helper.

use cardbench_magic_engine::{CardDefinition, Game, PlayerId};

#[test]
fn clear_event_log_cannot_refresh_a_seal_after_external_live_mutation() {
    let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("fixture initializes");
    game.begin_game().expect("game begins");
    game.players[PlayerId(0).0].life = 13;

    // A reset may discard measured setup receipts, but it must not authorize
    // an unrelated public-field edit made before the reset.
    game.clear_event_log();

    assert!(
        game.validate_invariants().is_err(),
        "clear_event_log laundered a receipt-free live-state mutation",
    );
}
