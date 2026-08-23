//! Red regression for Terraformer's omitted controller-land type activation.

use cardbench_magic_engine::{Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};

#[test]
fn terraformer_requires_its_chosen_basic_land_type_activation_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TERRAFORMER")
        .expect("Terraformer definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Blue])
    );
    assert!(
        definition
            .supported_rules
            .contains(&"controller-lands-chosen-basic-land-type-until-end-of-turn")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-TERRAFORMER"
            && binding.ability.id == "choose-controller-land-basic-type"
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
