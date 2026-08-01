//! Red discovery regression for Dowsing Shaman's graveyard-recursion activation.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, RAV_FULL_FIDELITY_DEFINITION_IDS,
};

#[test]
fn dowsing_shaman_requires_its_typed_enchantment_return_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DOWSING-SHAMAN")
        .expect("Dowsing Shaman definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the complete targeted graveyard-recursion ability is required"
    );
    assert!(definition
        .supported_rules
        .contains(&"return-target-enchantment-from-graveyard"));

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-DOWSING-SHAMAN"
                && binding.ability.id == "return-target-enchantment-from-graveyard"
        })
        .expect("Dowsing Shaman typed graveyard-return binding exists");
    assert_eq!(
        ability.ability.mana_cost,
        ManaCost::with_colors(2, [Color::Green])
    );
    assert!(ability.ability.tap_cost);
}
