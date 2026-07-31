//! Red regression for a permanent's enter-the-battlefield triggered ability.
//!
//! The probe uses public RAV identifiers and observable engine state only. It
//! does not retain card prose, art, or external database payloads.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn permanent_entry_queues_its_controller_trigger_before_priority_returns() {
    let controller = PlayerId(0);
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let caryatid = game
        .add_card(controller, "RAV-CARVEN-CARYATID", Zone::Hand)
        .expect("the green permanent begins in hand");
    let draw = game
        .add_card(controller, "RAV-FOREST", Zone::Library)
        .expect("one public card is available to draw");
    game.grant_mana(controller, Color::Green, 2)
        .expect("fixture provides colored mana");
    game.grant_mana(controller, Color::Red, 1)
        .expect("fixture provides generic mana");
    game.clear_event_log();

    game.cast_spell(
        controller,
        CastRequest {
            card: caryatid,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the permanent spell is legal");
    pass_to_top_resolution(&mut game);

    println!(
        "Carven ETB trigger red trace: stack={:?}; caryatid_zone={:?}; draw_zone={:?}; events={:?}",
        game.stack,
        game.zone_of(caryatid),
        game.zone_of(draw),
        game.canonical_event_log(),
    );
    assert_eq!(game.zone_of(caryatid), Some(Zone::Battlefield));
    assert_eq!(
        game.stack.len(),
        1,
        "a permanent entry must put its controller's triggered ability on the stack before any player regains priority"
    );
    assert_eq!(
        game.zone_of(draw),
        Some(Zone::Library),
        "the queued triggered ability has not resolved merely by being put on the stack"
    );
    game.validate_invariants()
        .expect("the trigger-queued state preserves engine invariants");
}

fn pass_to_top_resolution(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("priority holder passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player passes and the top spell resolves");
}
