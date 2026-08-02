use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn lurking_nullstone_manifest_has_all_positive_promotions() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 210);
    for id in [
        "RAV-LURKING-INFORMANT",
        "RAV-NULLSTONE-GARGOYLE",
        "RAV-GOLGARI-GERMINATION",
        "RAV-CLOUDSTONE-CURIO",
    ] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "missing {id}"
        );
    }
}
