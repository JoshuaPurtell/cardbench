use cardbench_magic_engine::{Game, PlayerId, Zone};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings,
};

fn game_with_static_bindings() -> Game {
    Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )
    .expect("RAV static-binding game builds")
}

#[test]
fn scion_sets_power_and_toughness_from_its_live_controller_creature_count() {
    let mut game = game_with_static_bindings();
    let scion = game
        .add_card(PlayerId(0), "RAV-SCION-OF-THE-WILD", Zone::Battlefield)
        .expect("Scion setup");
    assert_eq!(
        (
            game.characteristics(scion).unwrap().power,
            game.characteristics(scion).unwrap().toughness
        ),
        (Some(1), Some(1)),
    );

    game.add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("first controlled creature setup");
    assert_eq!(
        (
            game.characteristics(scion).unwrap().power,
            game.characteristics(scion).unwrap().toughness
        ),
        (Some(2), Some(2)),
    );

    game.add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("second controlled creature setup");
    assert_eq!(
        (
            game.characteristics(scion).unwrap().power,
            game.characteristics(scion).unwrap().toughness
        ),
        (Some(3), Some(3)),
    );
    game.validate_invariants()
        .expect("static binding invariants");
}

#[test]
fn scion_is_in_the_positive_full_fidelity_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SCION-OF-THE-WILD"));
}
