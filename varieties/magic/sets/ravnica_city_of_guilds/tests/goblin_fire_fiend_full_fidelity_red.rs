//! Ignored full-fidelity boundary probe for Goblin Fire Fiend.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Goblin Fire Fiend must-block and activated pump behavior are not implemented"]
fn goblin_fire_fiend_requires_its_omitted_behaviors_for_full_fidelity() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&fiend.id));
}
