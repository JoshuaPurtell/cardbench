//! Red regression probe for eliminated-seat leakage in policy-facing views.
//!
//! The expected visibility rule is specified before the engine projection is
//! corrected. A continuing multiplayer policy must not see, select, or rank a
//! player who has already left the game as an opponent.

use cardbench_magic_engine::{CardDefinition, Game, PlayerId};

#[test]
fn continuing_multiplayer_view_omits_eliminated_opponents() {
    let observer = PlayerId(0);
    let eliminated = PlayerId(1);
    let survivor = PlayerId(2);
    let mut game =
        Game::new(Vec::<CardDefinition>::new(), 3).expect("three-player fixture initializes");

    // Fixture setup creates a state-based-action boundary with two surviving
    // seats. The loss transition itself remains engine-owned and auditable.
    game.players[eliminated.0].life = 0;
    game.check_state_based_actions()
        .expect("zero life eliminates only that seat");
    assert!(
        game.player(eliminated)
            .expect("seat remains addressable")
            .lost
    );
    assert!(
        !game.is_game_over(),
        "two surviving players continue the game"
    );

    let view = game
        .view_for_player(observer)
        .expect("surviving player receives a policy view");

    assert_eq!(
        view.opponent_life,
        vec![(survivor, 20)],
        "an eliminated seat must not remain a policy-visible opponent; events: {:?}; view: {view:?}",
        game.event_log
    );
    assert!(
        view.opponent_battlefield.is_empty(),
        "a departed player must not contribute public battlefield data; view: {view:?}"
    );
    game.validate_invariants()
        .expect("the continuing multiplayer state remains internally valid");
}
