//! Ignored full-fidelity boundary probe for Woebringer Demon.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Woebringer Demon's upkeep sacrifice behavior is not implemented"]
fn woebringer_demon_requires_its_upkeep_trigger_for_full_fidelity() {
    let demon = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&demon.id));
}
