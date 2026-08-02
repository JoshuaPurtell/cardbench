//! Red merge-contract regression for Drake Familiar's promotion.

use cardbench_magic_rav::RAV_FULL_FIDELITY_DEFINITION_IDS;

#[test]
fn drake_familiar_promotion_reconciles_the_positive_manifest_cardinality() {
    assert_eq!(RAV_FULL_FIDELITY_DEFINITION_IDS.len(), 243);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-DRAKE-FAMILIAR"));
}
