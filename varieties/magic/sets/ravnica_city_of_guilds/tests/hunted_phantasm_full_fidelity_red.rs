//! Red regression for Hunted Phantasm's now-unblocked trigger target choice.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hunted_phantasm_requires_policy_selected_opponent_trigger_target_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-PHANTASM")
        .expect("Hunted Phantasm definition exists");

    assert!(
        definition
            .supported_rules
            .contains(&"policy-submitted-trigger-target")
    );
    assert!(
        !definition
            .supported_rules
            .contains(&"deterministic-opponent-target-selection"),
        "a full-fidelity targeted trigger cannot silently choose the first opponent"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
