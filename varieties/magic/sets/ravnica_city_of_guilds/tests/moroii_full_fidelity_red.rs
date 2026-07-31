//! Full-fidelity probe retained red until Moroii's upkeep trigger exists.

use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[ignore = "Moroii's upkeep life-loss trigger is not implemented"]
fn moroii_requires_upkeep_trigger_for_full_fidelity() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MOROII")
        .expect("Moroii definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
