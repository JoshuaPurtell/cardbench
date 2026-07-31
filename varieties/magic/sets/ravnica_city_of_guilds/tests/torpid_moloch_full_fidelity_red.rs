//! Full-fidelity contract for Torpid Moloch's activated ability.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn torpid_moloch_requires_its_land_sacrifice_activation_for_full_fidelity() {
    let moloch = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&moloch.id));
}
