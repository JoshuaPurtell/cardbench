//! Red discovery contract for Auratouched Mage's private Aura choice.
//!
//! The existing compatibility slice always chooses the first legal Aura in
//! library order.  A real controller may choose a different eligible Aura or
//! decline to find one, so that deterministic shortcut cannot be positive
//! full fidelity.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn auratouched_mage_requires_a_private_optional_compatible_aura_choice() {
    let mage = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AURATOUCHED-MAGE")
        .expect("Auratouched Mage definition exists");

    assert!(
        mage.supported_rules
            .contains(&"policy-selected-compatible-aura-selection"),
        "a deterministic first-match selector cannot represent the controller's choice"
    );
    assert!(
        mage.supported_rules
            .contains(&"optional-compatible-aura-search"),
        "the controller must be able to decline an otherwise legal Aura search"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&mage.id),
        "the exact choice boundary is required before Auratouched Mage can be full fidelity"
    );
}
