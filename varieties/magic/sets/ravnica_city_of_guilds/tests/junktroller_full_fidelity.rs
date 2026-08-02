//! Event-log contract for Junktroller's cross-owner graveyard activation.

use cardbench_magic_engine::{AbilityActivation, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds with Junktroller binding")
}

#[test]
fn junktroller_moves_an_opponents_graveyard_card_to_its_owners_library_bottom() {
    let mut game = game();
    let junktroller = game
        .put_on_battlefield(PlayerId(0), "RAV-JUNKTROLLER")
        .expect("Junktroller setup");
    game.set_entered_turn_for_setup(junktroller, 0)
        .expect("fixture makes Junktroller long-controlled");
    let target = game
        .add_card(PlayerId(1), "RAV-LIGHTNING-HELIX", Zone::Graveyard)
        .expect("opponent target setup");
    let existing_bottom = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Library)
        .expect("opponent library bottom setup");
    let existing_top = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Library)
        .expect("opponent library top setup");
    game.begin_game().expect("game begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: junktroller,
            ability_id: "tap-target-graveyard-card-to-owners-library-bottom",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("opponent graveyard card is a legal target");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(game.object(junktroller).expect("source remains").tapped);
    assert_eq!(game.zone_of(target), Some(Zone::Library));
    assert_eq!(
        game.players[1].library,
        vec![target, existing_bottom, existing_top],
        "the targeted card is placed beneath every existing owner-library card"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved { card, to: Zone::Library } if *card == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved {
            source,
            ability: "tap-target-graveyard-card-to-owners-library-bottom",
            ..
        } if *source == junktroller
    )));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-JUNKTROLLER"));
    eprintln!("junktroller_trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Junktroller graveyard move remains invariant-valid");
}
