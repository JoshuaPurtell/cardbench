//! Ignored full-fidelity boundary probe for Carven Caryatid.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Carven Caryatid's enter-the-battlefield draw trigger is not implemented"]
fn carven_caryatid_requires_its_draw_trigger_for_full_fidelity() {
    let caryatid = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARVEN-CARYATID")
        .expect("Carven Caryatid definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&caryatid.id));
}
