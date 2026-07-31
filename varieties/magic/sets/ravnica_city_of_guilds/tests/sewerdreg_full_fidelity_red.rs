//! Full-fidelity probe retained red until Sewerdreg regeneration exists.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Sewerdreg's activated regeneration shield is not implemented"]
fn sewerdreg_requires_regeneration_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SEWERDREG")
        .expect("Sewerdreg definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Sewerdreg cannot claim full fidelity without regeneration"
    );
}
