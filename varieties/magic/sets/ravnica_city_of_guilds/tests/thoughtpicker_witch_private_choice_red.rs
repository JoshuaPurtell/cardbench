//! Red discovery contract for Thoughtpicker Witch's opponent-library choice.
//!
//! The card is currently only a base creature chassis.  This contract makes
//! the missing stack-backed, controller-private choice explicit before the
//! current engine substrate is extended to represent it.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn thoughtpicker_witch_requires_private_opponent_library_exile_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-THOUGHTPICKER-WITCH")
        .expect("Thoughtpicker Witch definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the sacrifice, target-opponent, private-library-choice activation is required"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"private-opponent-library-exile-choice"),
        "the executable definition must declare its private choice boundary"
    );
}
