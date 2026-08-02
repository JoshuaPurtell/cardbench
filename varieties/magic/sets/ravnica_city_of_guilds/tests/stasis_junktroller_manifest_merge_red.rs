//! Merge regression: Stasis Cell and Junktroller must share one manifest count.

use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn combined_manifest_has_stasis_and_junktroller() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 184);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-STASIS-CELL"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-JUNKTROLLER"));
}
