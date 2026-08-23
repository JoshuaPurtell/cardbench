//! Red contract for Dimir Machinations' target-library reorder face.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dimir_machinations_needs_private_target_library_reordering() {
    let machinations = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-MACHINATIONS")
        .expect("Dimir Machinations definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&machinations.id),
        "Dimir Machinations cannot be complete while its target-library face is absent"
    );
    assert!(
        machinations
            .supported_rules
            .contains(&"private-target-player-top-three-library-reorder"),
        "the full definition must expose its private target-player top-three reorder rule"
    );
}
