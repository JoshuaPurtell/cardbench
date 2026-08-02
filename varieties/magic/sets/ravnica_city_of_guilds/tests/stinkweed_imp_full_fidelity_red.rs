//! Positive-manifest regression for ability-complete Stinkweed Imp.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn stinkweed_imp_includes_its_damage_trigger_for_full_fidelity() {
    let imp = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STINKWEED-IMP")
        .expect("Stinkweed Imp definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&imp.id));
}
