use cardbench_magic_engine::{AbilityActivation, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_activated_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn greater_forgeling_pump_is_a_stack_ability() {
    let mut game = game_with_activated_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-GREATER-FORGELING")
        .expect("Greater Forgeling enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old fixture entry");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("generic-compatible mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "pump-plus-three-minus-three",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("pump activation");
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("pump resolves");
    let characteristics = game.characteristics(source).expect("live characteristics");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(6), Some(1))
    );
}

#[test]
fn viashino_slasher_pump_is_a_stack_ability() {
    let mut game = game_with_activated_bindings();
    let source = game
        .put_on_battlefield(PlayerId(0), "RAV-VIASHINO-SLASHER")
        .expect("Viashino Slasher enters");
    game.set_entered_turn_for_setup(source, 0)
        .expect("old fixture entry");
    let discarded = game
        .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
        .expect("discardable card enters hand");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "pump-plus-one-minus-one",
            sacrifice_sources: vec![],
            discard_cards: vec![discarded],
            targets: vec![],
        },
    )
    .expect("pump activation");
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("pump resolves");
    let characteristics = game.characteristics(source).expect("live characteristics");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(2), Some(1))
    );
}
