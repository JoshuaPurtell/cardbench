//! Merge regression: two independently promoted cards must share one manifest count.

use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn combined_spectral_searchlight_and_terrarion_manifest_has_both_promotions() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 209);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SPECTRAL-SEARCHLIGHT"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-TERRARION"));
}
