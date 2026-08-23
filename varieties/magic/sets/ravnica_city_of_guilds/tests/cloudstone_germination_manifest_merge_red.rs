use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

/// This integration regression is intentionally added before the count repair.
/// Germination and Cloudstone are both positive promotions, so the manifest
/// must contain both without silently retaining the preceding cardinality.
#[test]
fn germination_then_cloudstone_manifest_has_both_promotions() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-GOLGARI-GERMINATION"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CLOUDSTONE-CURIO"));
}
