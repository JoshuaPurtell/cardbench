//! Red merge-contract regression for the Woebringer promotion.

use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn woebringer_promotion_reconciles_the_positive_manifest_cardinality() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 226);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-WOEBRINGER-DEMON"));
}
