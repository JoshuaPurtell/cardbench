use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn dark_heart_keeps_the_combined_full_manifest() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 210);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DARK-HEART-OF-THE-WOOD"));
}
