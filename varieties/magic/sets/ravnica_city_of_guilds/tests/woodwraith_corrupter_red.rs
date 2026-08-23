//! Red contract for Woodwraith Corrupter's persistent Forest-animation ability.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn woodwraith_corrupter_needs_its_typed_persistent_forest_animation() {
    let corrputer = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOODWRAITH-CORRUPTER")
        .expect("Woodwraith Corrupter definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&corrputer.id),
        "Woodwraith Corrupter cannot be complete while its persistent Forest animation is absent"
    );
    assert!(
        corrputer
            .supported_rules
            .contains(&"activated-persistent-forest-animation"),
        "Woodwraith Corrupter must expose the persistent Forest animation rule"
    );
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == corrputer.id)
        .expect("Woodwraith Corrupter animated-Forest binding exists");
    assert_eq!(binding.ability.id, "animate-target-forest");
    assert_eq!(
        binding.ability.mana_cost,
        ManaCost::with_colors(1, [Color::Black, Color::Green])
    );
    assert!(binding.ability.tap_cost);
    assert_eq!(binding.ability.targets.len(), 1);
}
