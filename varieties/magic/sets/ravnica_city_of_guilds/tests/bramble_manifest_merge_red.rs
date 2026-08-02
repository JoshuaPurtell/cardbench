use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn bramble_promotion_reconciles_the_positive_manifest_count() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 223);
}
