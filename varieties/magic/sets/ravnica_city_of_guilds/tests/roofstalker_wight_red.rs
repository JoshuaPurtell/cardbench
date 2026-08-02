//! Red discovery regression for Roofstalker Wight's self-Flying activation.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn roofstalker_wight_requires_its_black_cast_cost_and_blue_self_flying_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ROOFSTALKER-WIGHT")
        .expect("Roofstalker Wight definition exists");

    assert_eq!(definition.name, "Roofstalker Wight");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into());
    assert_eq!(definition.power, Some(2));
    assert_eq!(definition.toughness, Some(1));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"self-flying-until-end-of-turn")
    );

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-ROOFSTALKER-WIGHT"
                && binding.ability.id == "blue-gain-flying"
        })
        .expect("Roofstalker Wight self-Flying binding exists");
    assert_eq!(
        ability.ability.mana_cost,
        ManaCost::with_colors(1, [Color::Blue])
    );
    assert!(!ability.ability.tap_cost);
    assert!(ability.ability.targets.is_empty());
    assert_eq!(ability.ability.effects.len(), 1);
}
