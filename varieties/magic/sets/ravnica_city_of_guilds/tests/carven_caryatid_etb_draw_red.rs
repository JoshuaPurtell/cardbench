//! Red regression for Carven Caryatid's enter-the-battlefield draw trigger.
//!
//! Resolving the creature must put its controller's draw trigger on the stack
//! and make the resulting draw visible in the event log.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{card_definitions, rav_triggered_ability_bindings};

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture starts");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active pass");
        game.pass_priority(PlayerId(1)).expect("response pass");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn carven_caryatid_enters_and_draws_through_a_stack_trigger() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
        std::iter::empty(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger-enabled catalog builds");
    let caryatid = game
        .add_card(PlayerId(0), "RAV-CARVEN-CARYATID", Zone::Hand)
        .expect("Caryatid begins in hand");
    let library_card = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("library card exists");
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Green, 3)
        .expect("green mana added during the main phase");
    game.clear_event_log();

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
    game.pass_priority(PlayerId(1)).expect("opponent passes");
    game.pass_priority(PlayerId(0))
        .expect("controller passes ETB trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes ETB trigger");

    println!("Carven Caryatid trigger trace: {:?}", game.event_log);
    assert_eq!(game.zone_of(caryatid), Some(Zone::Battlefield));
    assert_eq!(
        game.zone_of(library_card),
        Some(Zone::Hand),
        "Carven Caryatid's ETB trigger must draw its controller a card"
    );
    assert!(
        game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == library_card)),
        "the ETB trigger must leave a CardMoved receipt for the drawn card"
    );
    assert_eq!(
        game.stack.len(),
        0,
        "the trigger resolves from the shared stack"
    );
    game.validate_invariants()
        .expect("ETB trigger resolution preserves engine invariants");
}
