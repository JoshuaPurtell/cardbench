//! Red reconciliation probe for Consult's full-fidelity classification.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn consult_private_discard_completion_promotes_the_card_to_the_positive_manifest() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CONSULT-THE-NECROSAGES")
        .expect("Consult the Necrosages definition exists");
    assert!(
        definition.supported_rules.contains(&"full-rules-fidelity"),
        "all printed Consult modes now resolve through typed, audited paths"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the full-fidelity manifest must include Consult once recipient-private discard is complete"
    );
}
