//! Red contract for Shred Memory's variable graveyard-target front face.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn shred_memory_needs_its_up_to_four_same_graveyard_exile_face() {
    let shred = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SHRED-MEMORY")
        .expect("Shred Memory definition exists");

    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&shred.id),
        "Shred Memory cannot be complete while its variable graveyard exile face is absent"
    );
    assert!(
        shred
            .supported_rules
            .contains(&"exile-up-to-four-target-cards-single-graveyard"),
        "the full definition must expose the same-graveyard variable target rule"
    );
}
