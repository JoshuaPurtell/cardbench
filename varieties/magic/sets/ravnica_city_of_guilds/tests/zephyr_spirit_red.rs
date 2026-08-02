//! Red regression for Zephyr Spirit's omitted blocker trigger.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn zephyr_spirit_requires_its_blocks_return_trigger_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ZEPHYR-SPIRIT")
        .expect("Zephyr Spirit definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Blue])
    );
    assert!(
        definition
            .supported_rules
            .contains(&"blocks-return-source-owner-hand")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-ZEPHYR-SPIRIT"
            && binding.ability.id == "blocks-return-source-owner-hand"
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
