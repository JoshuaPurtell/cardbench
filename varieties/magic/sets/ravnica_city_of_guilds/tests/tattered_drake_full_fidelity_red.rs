//! Ignored full-fidelity boundary probe for Tattered Drake.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Tattered Drake's regeneration activation is not implemented"]
fn tattered_drake_requires_regeneration_for_full_fidelity() {
    let drake = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TATTERED-DRAKE")
        .expect("Tattered Drake definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&drake.id));
}
