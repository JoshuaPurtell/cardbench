//! Ignored full-fidelity boundary probe for Screeching Griffin.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Screeching Griffin's activated ability is not implemented"]
fn screeching_griffin_requires_its_activated_ability_for_full_fidelity() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
}
