//! Red regression: terminal setup-to-live transition must be rejected atomically.

use cardbench_magic_engine::{Game, PlayerId};

#[test]
fn begin_game_rejects_a_terminal_fixture_without_appending_turn_events() {
    let survivor = PlayerId(0);
    let eliminated = PlayerId(1);
    let mut game = Game::new(Vec::new(), 2).expect("two-player fixture initializes");
    game.players[eliminated.0].life = 0;
    game.check_state_based_actions()
        .expect("public SBA seam establishes the terminal lifecycle");
    assert!(game.is_game_over(), "fixture must be terminal before start");
    let events_before = game.event_log.clone();
    let active_before = game.active_player;
    let priority_before = game.priority;
    let step_before = game.step;
    let turn_before = game.turn;

    let result = game.begin_game();

    assert!(result.is_err(), "a terminal fixture must not start a new game");
    assert_eq!(
        game.event_log, events_before,
        "rejected terminal begin_game appended turn events; result={result:?}; before={events_before:?}; after={:?}; invariant={:?}",
        game.event_log,
        game.validate_invariants(),
    );
    assert_eq!(game.active_player, active_before, "rejected start changed active player");
    assert_eq!(game.priority, priority_before, "rejected start changed priority");
    assert_eq!(game.step, step_before, "rejected start changed step");
    assert_eq!(game.turn, turn_before, "rejected start changed turn");
    assert_eq!(game.winner(), Some(survivor));
    game.validate_invariants()
        .expect("rejected terminal start must preserve a valid terminal game");
}
