use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn shock_lands_share_the_combined_full_manifest() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 192);
    for id in [
        "RAV-SACRED-FOUNDRY",
        "RAV-TEMPLE-GARDEN",
        "RAV-WATERY-GRAVE",
    ] {
        assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id));
    }
}
