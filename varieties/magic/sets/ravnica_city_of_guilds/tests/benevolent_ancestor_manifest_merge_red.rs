use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn benevolent_ancestor_promotion_updates_the_positive_manifest_count() {
    // This intentionally captures the stale pre-promotion contract. The green
    // card promotion adds Benevolent Ancestor before the count reconciliation.
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 209);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-BENEVOLENT-ANCESTOR"));
}
