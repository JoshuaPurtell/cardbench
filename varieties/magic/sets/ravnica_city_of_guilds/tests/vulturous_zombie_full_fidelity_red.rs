//! Full-fidelity probe retained red until Vulturous Zombie's graveyard trigger exists.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Vulturous Zombie's graveyard counter trigger is not implemented"]
fn vulturous_zombie_requires_graveyard_trigger_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VULTUROUS-ZOMBIE")
        .expect("Vulturous Zombie definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
