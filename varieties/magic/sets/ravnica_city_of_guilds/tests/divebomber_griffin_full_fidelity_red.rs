//! Ignored full-fidelity boundary probe for Divebomber Griffin.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Divebomber Griffin's activated sacrifice/damage ability is not implemented"]
fn divebomber_griffin_requires_its_activation_for_full_fidelity() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
}
