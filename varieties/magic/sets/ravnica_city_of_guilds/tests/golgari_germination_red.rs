//! Red discovery contract for Golgari Germination's controlled nontoken dies trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Effect, ManaCost, TokenSpec, TriggerCondition,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn golgari_germination_requires_a_controlled_nontoken_creature_dies_trigger() {
    let germination = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOLGARI-GERMINATION")
        .expect("Golgari Germination definition exists");

    assert_eq!(germination.name, "Golgari Germination");
    assert_eq!(
        germination.mana_cost,
        ManaCost::with_colors(1, [Color::Black, Color::Green])
    );
    assert_eq!(
        germination.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(germination.card_types, BTreeSet::from([CardType::Enchantment]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&germination.id));
    assert!(germination
        .supported_rules
        .contains(&"controlled-nontoken-creature-dies-create-saproling"));

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-GOLGARI-GERMINATION")
        .expect("Golgari Germination trigger binding exists");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::ControlledNontokenCreatureDies
    );
    assert_eq!(
        trigger.ability.effects,
        [Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 1,
        }]
    );
}
