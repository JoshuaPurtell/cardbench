use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn sunforger_bottled_manifest_has_all_positive_promotions() {
    for id in [
        "RAV-SUNFORGER",
        "RAV-BOTTLED-CLOISTER",
        "RAV-PARIAHS-SHIELD",
        "RAV-LURKING-INFORMANT",
        "RAV-NULLSTONE-GARGOYLE",
    ] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "missing {id}"
        );
    }
}
