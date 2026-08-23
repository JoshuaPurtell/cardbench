//! Red contract for Perplex's policy-owned counter-or-discard resolution.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn perplex_needs_its_counter_unless_controller_discards_hand_face() {
    let perplex = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-PERPLEX")
        .expect("Perplex definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&perplex.id),
        "Perplex cannot be complete while its counter-or-discard face is absent"
    );
    assert!(
        perplex
            .supported_rules
            .contains(&"counter-target-spell-unless-controller-discards-hand"),
        "Perplex must expose its policy-owned discard-hand counter alternative"
    );
}
