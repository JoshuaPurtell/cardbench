use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn grave_troll_promotion_updates_the_positive_manifest_count() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GOLGARI-GRAVE-TROLL"));
}
