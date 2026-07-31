//! Ignored full-fidelity boundary probe for Grozoth.
//!
//! Grozoth's entry trigger is not represented and the existing Transmute
//! substrate resolves immediately instead of using the stack. This test stays
//! ignored until both gaps are closed, while keeping the missing scope visible.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Grozoth's entry trigger and stack-backed Transmute are not implemented"]
fn grozoth_requires_entry_trigger_and_stack_backed_transmute_for_full_fidelity() {
    let grozoth = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GROZOTH")
        .expect("Grozoth definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&grozoth.id),
        "full fidelity requires the omitted entry trigger and a response window"
    );
}
