//! Red discovery contract for Wojek Apothecary's Radiance prevention ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, Game, ManaCost, PlayerId, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn wojek_apothecary_has_its_radiance_damage_prevention_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOJEK-APOTHECARY")
        .expect("Wojek Apothecary definition exists");
    assert_eq!(definition.name, "Wojek Apothecary");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-radiance-prevent-one-damage")
    );
}

#[test]
fn wojek_apothecary_can_stack_its_targeted_radiance_prevention_ability() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let apothecary = game
        .put_on_battlefield(PlayerId(0), "RAV-WOJEK-APOTHECARY")
        .expect("Wojek Apothecary setup");
    let target = game
        .put_on_battlefield(PlayerId(0), "RAV-BLAZING-ARCHON")
        .expect("colored creature setup");
    game.set_entered_turn_for_setup(apothecary, 0)
        .expect("old source");
    game.set_entered_turn_for_setup(target, 0)
        .expect("old target");
    game.begin_game().expect("game begins");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: apothecary,
            ability_id: "tap-radiance-prevent-one-damage",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("Radiance prevention ability stacks");
}
