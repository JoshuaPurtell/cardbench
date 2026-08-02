//! Red regression for Disembowel's missing public policy-action coverage.
//!
//! The engine already retains a policy-proposed X value on the spell stack and
//! validates the targeted creature against it.  The shown scenario grammar
//! cannot submit that legal action, so the card must not be promoted until the
//! public policy boundary is wired and exercised.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn disembowel_is_not_left_partial_when_its_existing_x_cast_action_is_policy_reachable() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DISEMBOWEL")
        .expect("Disembowel definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Disembowel has an invariant-checked policy X cast path but is not full-fidelity"
    );
    assert!(definition.supported_rules.contains(&"full-rules-fidelity"));
}
