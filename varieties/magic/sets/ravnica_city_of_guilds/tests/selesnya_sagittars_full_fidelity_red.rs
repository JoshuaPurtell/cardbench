//! Ignored full-fidelity boundary probe for Selesnya Sagittars.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Selesnya Sagittars' tap-to-damage activation is not implemented"]
fn selesnya_sagittars_requires_its_activated_damage_ability_for_full_fidelity() {
    let sagittars = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-SAGITTARS")
        .expect("Selesnya Sagittars definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sagittars.id));
}
