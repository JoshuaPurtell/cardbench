//! Red regression for the four RAV guild bounce lands' semantic boundary.
//!
//! Their shared land-entry, policy-selected return trigger, and fixed mana
//! bundle are already executable. They must not remain a bounded chassis once
//! that complete common behavior is published in the positive manifest.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn requires_full_fidelity(id: &str) {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .expect("guild bounce land definition exists");
    assert!(
        definition.supported_rules.contains(&"full-rules-fidelity"),
        "{id} must explicitly publish the complete shared land slice"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "{id} must be in the positive full-fidelity manifest"
    );
}

#[test]
fn boros_garrison_is_not_left_as_a_bounded_chassis() {
    requires_full_fidelity("RAV-BOROS-GARRISON");
}

#[test]
fn dimir_aqueduct_is_not_left_as_a_bounded_chassis() {
    requires_full_fidelity("RAV-DIMIR-AQUEDUCT");
}

#[test]
fn golgari_rot_farm_is_not_left_as_a_bounded_chassis() {
    requires_full_fidelity("RAV-GOLGARI-ROT-FARM");
}

#[test]
fn selesnya_sanctuary_is_not_left_as_a_bounded_chassis() {
    requires_full_fidelity("RAV-SELESNYA-SANCTUARY");
}
