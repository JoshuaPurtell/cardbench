use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn nullstone_germination_cloudstone_manifest_has_all_promotions() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 209);
    for id in [
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
