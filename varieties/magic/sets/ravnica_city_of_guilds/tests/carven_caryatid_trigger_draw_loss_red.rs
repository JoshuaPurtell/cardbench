//! Red regression for a terminal draw caused by a stack-backed ETB trigger.
//!
//! A Carven Caryatid trigger that resolves against an empty library eliminates
//! its controller. The trigger's terminal receipt must not be appended after
//! `GameEnded`, or the all-or-error priority transition rejects the otherwise
//! legal loss and rolls the trigger resolution back.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Step, Zone};
use cardbench_magic_rav::{card_definitions, rav_trigger_bindings};

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("response pass");
    }
    assert_eq!(game.step, Step::PrecombatMain);
}

#[test]
fn carven_caryatid_trigger_draw_loss_is_atomic_and_terminal() {
    let mut game = Game::new_with_triggers(card_definitions(), 2, rav_trigger_bindings())
        .expect("RAV trigger-enabled catalog builds");
    let caryatid = game
        .add_card(PlayerId(0), "RAV-CARVEN-CARYATID", Zone::Hand)
        .expect("Caryatid begins in hand");
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Green, 3)
        .expect("green mana pays Caryatid");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: caryatid,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Caryatid casts");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes spell");
    game.pass_priority(PlayerId(0))
        .expect("controller passes trigger");

    let result = game.pass_priority(PlayerId(1));
    println!("Carven trigger draw-loss result: {result:?}");
    println!("Carven trigger draw-loss trace: {:?}", game.event_log);
    assert!(
        result.is_ok(),
        "a legal empty-library trigger draw must complete the player-loss transition"
    );
    assert!(game.is_game_over());
    assert!(matches!(
        game.event_log.last(),
        Some(cardbench_magic_engine::GameEvent::GameEnded { .. })
    ));
    game.validate_invariants()
        .expect("terminal trigger draw preserves invariants");
}
