//! Red contract regression for Carven Caryatid's complete trigger slice.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn carven_caryatid_requires_its_draw_trigger_for_full_fidelity() {
    let caryatid = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARVEN-CARYATID")
        .expect("Carven Caryatid definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&caryatid.id));
    assert!(
        caryatid
            .supported_rules
            .contains(&"enter-the-battlefield-draw")
    );
}
