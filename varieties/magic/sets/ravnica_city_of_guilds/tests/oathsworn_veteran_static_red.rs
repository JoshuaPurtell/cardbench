//! Red discovery contract for the White static-creature source-lane path.

use cardbench_magic_engine::{CardType, Color, Game, Keyword, ManaCost, PlayerId, Zone};
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
fn oathsworn_giant_and_veteran_armorer_require_complete_other_creature_static_effects() {
    let definitions = card_definitions();
    let oathsworn = definitions
        .iter()
        .find(|definition| definition.id == "RAV-OATHSWORN-GIANT")
        .expect("Oathsworn Giant definition exists");
    assert_eq!(
        oathsworn.mana_cost,
        ManaCost::with_colors(4, [Color::White, Color::White])
    );
    assert_eq!(oathsworn.card_types, [CardType::Creature].into());
    assert_eq!((oathsworn.power, oathsworn.toughness), (Some(3), Some(4)));
    assert_eq!(oathsworn.keywords, [Keyword::Vigilance]);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&oathsworn.id));
    assert!(
        oathsworn
            .supported_rules
            .contains(&"static-other-creatures-vigilance-plus-zero-two")
    );

    let veteran = definitions
        .iter()
        .find(|definition| definition.id == "RAV-VETERAN-ARMORER")
        .expect("Veteran Armorer definition exists");
    assert_eq!(veteran.mana_cost, ManaCost::with_colors(1, [Color::White]));
    assert_eq!(veteran.card_types, [CardType::Creature].into());
    assert_eq!((veteran.power, veteran.toughness), (Some(2), Some(2)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&veteran.id));
    assert!(
        veteran
            .supported_rules
            .contains(&"static-other-creatures-plus-zero-one")
    );

    let mut game = game_with_static_bindings();
    let veteran = game
        .add_card(PlayerId(0), "RAV-VETERAN-ARMORER", Zone::Battlefield)
        .expect("Veteran setup");
    let oathsworn = game
        .add_card(PlayerId(0), "RAV-OATHSWORN-GIANT", Zone::Battlefield)
        .expect("Oathsworn setup");
    let ally = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("friendly creature setup");
    let opponent = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("opposing creature setup");

    let veteran_characteristics = game.characteristics(veteran).expect("Veteran remains live");
    assert_eq!(
        (
            veteran_characteristics.power,
            veteran_characteristics.toughness
        ),
        (Some(2), Some(4)),
        "Oathsworn affects another controlled creature, including Veteran"
    );
    assert!(
        veteran_characteristics
            .keywords
            .contains(&Keyword::Vigilance)
    );
    let oathsworn_characteristics = game
        .characteristics(oathsworn)
        .expect("Oathsworn remains live");
    assert_eq!(
        (
            oathsworn_characteristics.power,
            oathsworn_characteristics.toughness
        ),
        (Some(3), Some(5)),
        "Veteran affects another controlled creature but Oathsworn does not affect itself"
    );
    assert_eq!(
        game.characteristics(ally)
            .expect("friendly creature remains live")
            .toughness,
        Some(6),
        "both same-controller static modifiers stack"
    );
    assert!(
        game.characteristics(ally)
            .expect("friendly creature remains live")
            .keywords
            .contains(&Keyword::Vigilance)
    );
    let opponent_characteristics = game
        .characteristics(opponent)
        .expect("opposing creature remains live");
    assert_eq!(
        (
            opponent_characteristics.power,
            opponent_characteristics.toughness
        ),
        (Some(3), Some(3)),
        "controller-scoped static effects do not cross seats"
    );
    assert!(
        !opponent_characteristics
            .keywords
            .contains(&Keyword::Vigilance)
    );
    game.validate_invariants()
        .expect("static continuous effects preserve engine invariants");
}
