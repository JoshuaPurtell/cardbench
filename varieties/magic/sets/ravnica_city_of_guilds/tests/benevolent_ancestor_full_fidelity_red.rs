//! Full-fidelity manifest regression for Benevolent Ancestor.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn benevolent_ancestor_requires_its_prevention_activation_for_full_fidelity() {
    let ancestor = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BENEVOLENT-ANCESTOR")
        .expect("Benevolent Ancestor definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&ancestor.id));
}
