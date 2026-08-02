//! Red regression probes for small RAV multicolor/artifact cards.
//!
//! These cards are intentionally exercised through the expansion-neutral
//! trigger, activated-ability, and attachment substrates rather than through
//! card-name branches.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn centaur_safeguard_dies_life_gain_is_full_fidelity() {
    let safeguard = definition("RAV-CENTAUR-SAFEGUARD");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&safeguard.id),
        "Centaur Safeguard must be in the full-fidelity manifest"
    );
}

#[test]
fn cyclopean_snare_tap_activation_is_full_fidelity() {
    let snare = definition("RAV-CYCLOPEAN-SNARE");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&snare.id),
        "Cyclopean Snare must be in the full-fidelity manifest"
    );
}

#[test]
fn grifters_blade_equipment_attachment_is_full_fidelity() {
    let blade = definition("RAV-GRIFTERS-BLADE");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&blade.id),
        "Grifter's Blade must be in the full-fidelity manifest"
    );
}
