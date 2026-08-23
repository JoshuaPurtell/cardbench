//! Red merge-contract regression for the Vulturous Zombie promotion.

use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn vulturous_promotion_reconciles_the_positive_manifest_cardinality() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 291);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-VULTUROUS-ZOMBIE"));
}
