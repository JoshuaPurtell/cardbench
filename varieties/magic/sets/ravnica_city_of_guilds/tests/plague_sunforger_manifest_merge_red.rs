use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn plague_sunforger_manifest_has_all_positive_promotions() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 209);
    for id in [
        "RAV-PLAGUE-BOILER",
        "RAV-SUNFORGER",
        "RAV-BOTTLED-CLOISTER",
        "RAV-PARIAHS-SHIELD",
        "RAV-LURKING-INFORMANT",
    ] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
            "missing {id}"
        );
    }
}
