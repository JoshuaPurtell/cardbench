use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn crown_reroute_and_overgrown_tomb_share_the_reconciled_full_manifest() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 187);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CROWN-OF-CONVERGENCE"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-OVERGROWN-TOMB"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-REROUTE"));
}
