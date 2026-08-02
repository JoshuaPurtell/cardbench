use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn brownscale_promotion_updates_the_positive_manifest_count() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 210);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GOLGARI-BROWNSCALE"));
}
