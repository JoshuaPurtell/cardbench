//! Red regression for Civic Wayfinder's selected-land reveal receipt.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn civic_wayfinder_reveals_the_land_selected_from_its_library() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds");
    game.step = Step::PrecombatMain;
    let wayfinder = game
        .add_card(PlayerId(0), "RAV-CIVIC-WAYFINDER", Zone::Hand)
        .expect("Wayfinder setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("controller basic land setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("Wayfinder mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: wayfinder,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Wayfinder casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Wayfinder resolves");
    game.pass_priority(PlayerId(0))
        .expect("controller passes ETB trigger");
    game.pass_priority(PlayerId(1))
        .expect("ETB trigger resolves");

    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed {
            player: PlayerId(0),
            card,
            definition: "RAV-FOREST",
        } if *card == forest
    )));
}
