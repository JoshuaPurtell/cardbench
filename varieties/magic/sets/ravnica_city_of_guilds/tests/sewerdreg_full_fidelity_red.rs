//! Red regression for Sewerdreg's missing activated regeneration ability.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
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
