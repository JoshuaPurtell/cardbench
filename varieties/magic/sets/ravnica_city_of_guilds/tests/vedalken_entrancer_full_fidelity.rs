//! Event-log contract for Vedalken Entrancer's targeted milling activation.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, ManaCost, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn entrancer_pays_blue_taps_and_mills_the_chosen_player_two_cards() {
    let mut game = game_with_rav_bindings();
    let entrancer = game
        .put_on_battlefield(PlayerId(0), "RAV-VEDALKEN-ENTRANCER")
        .expect("Entrancer setup");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("Island setup");
    let first_milled = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .expect("first library card");
    let second_milled = game
        .add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Library)
        .expect("second library card");
    game.set_entered_turn_for_setup(entrancer, 0)
        .expect("Entrancer entered on an earlier turn");

    game.begin_game().expect("game begins");
    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("Island produces Blue");
    game.clear_event_log();
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: entrancer,
            ability_id: "tap-blue-mill-two",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Entrancer ability uses stack");
    assert!(game.object(entrancer).expect("Entrancer exists").tapped);
    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second).expect("activation resolves");
    println!(
        "vedalken_entrancer_event_log={:#?}",
        game.canonical_event_log()
    );

    for card in [first_milled, second_milled] {
        assert_eq!(game.zone_of(card), Some(Zone::Graveyard));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityManaPaid {
            source,
            ability,
            mana_cost,
            ..
        } if *source == entrancer
            && *ability == "tap-blue-mill-two"
            && *mana_cost == ManaCost::with_colors(0, [Color::Blue])
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == entrancer && *ability == "tap-blue-mill-two"
    )));
    game.validate_invariants()
        .expect("Entrancer event trace preserves invariants");
}
