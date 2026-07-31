//! Red milestone for Indentured Oaf's source-color damage prevention.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn indentured_oaf_declares_red_damage_prevention() {
    let oaf = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-INDENTURED-OAF")
        .expect("Indentured Oaf definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&oaf.id));
    assert!(oaf
        .supported_rules
        .contains(&"prevent-damage-from-red-sources"));
}
