//! Regression probe for priority reset after a public mana action.

use cardbench_magic_engine::{CardDefinition, Color, Game, GameEvent, PlayerId, Step};

#[test]
fn public_mana_action_resets_the_pass_sequence_before_a_second_pass() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game =
        Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player fixture initializes");
    game.begin_game().expect("game reaches upkeep priority");

    game.pass_priority(first)
        .expect("first player passes to the second player");
    assert_eq!(game.priority, second);
    game.add_mana_from_action(second, Color::Blue, 1)
        .expect("the priority holder takes a public mana action");
    game.pass_priority(second)
        .expect("the mana-action player may then pass");

    assert_eq!(
        game.step,
        Step::Upkeep,
        "a mana action interrupts the pass sequence, so the first player must receive a response window instead of advancing the step; events: {:?}",
        game.event_log
    );
    assert_eq!(game.priority, first);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ManaAdded { player, color: Color::Blue, amount: 1 } if *player == second
    )));
    game.validate_invariants()
        .expect("the restored response window remains internally valid");
}
