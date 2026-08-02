use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn overgrown_tomb_and_reroute_share_the_reconciled_full_manifest() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 186);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-OVERGROWN-TOMB"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-REROUTE"));
}
