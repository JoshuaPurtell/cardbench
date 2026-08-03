//! Red discovery contract for Brightflame's coupled Radiance damage and life gain.

use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn brightflame_requires_an_executable_chosen_x_radiance_damage_life_definition() {
    assert_eq!(
        executable_definition_id_for_collector(194),
        Ok("RAV-BRIGHTFLAME"),
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BRIGHTFLAME")
        .expect("Brightflame definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"radiance-chosen-x-damage-total-life-gain")
    );
}
