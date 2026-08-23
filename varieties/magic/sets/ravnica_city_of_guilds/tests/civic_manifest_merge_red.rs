use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn civic_wayfinder_remains_in_the_positive_manifest() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CIVIC-WAYFINDER"));
}
