//! Red milestone for the remaining Goblin Fire Fiend rules.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn goblin_fire_fiend_requires_its_omitted_behaviors_for_full_fidelity() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&fiend.id));
}

#[test]
fn goblin_fire_fiend_declares_must_block_and_pump_rules() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");
    assert!(fiend
        .supported_rules
        .contains(&"must-block-if-able"));
    assert!(fiend
        .supported_rules
        .contains(&"activated-plus-one-power"));
}
