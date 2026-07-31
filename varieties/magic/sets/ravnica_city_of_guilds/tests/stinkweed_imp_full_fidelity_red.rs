//! Ignored full-fidelity boundary probe for Stinkweed Imp.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Stinkweed Imp's damage-triggered destruction behavior is not implemented"]
fn stinkweed_imp_requires_its_damage_trigger_for_full_fidelity() {
    let imp = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STINKWEED-IMP")
        .expect("Stinkweed Imp definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&imp.id));
}
