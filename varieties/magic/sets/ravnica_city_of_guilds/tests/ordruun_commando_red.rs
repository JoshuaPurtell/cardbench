//! Red milestone for Ordruun Commando's damage-prevention activation.

use cardbench_magic_rav::{card_definitions, RAV_FULL_FIDELITY_DEFINITION_IDS};

#[test]
fn ordruun_commando_is_full_fidelity_and_declares_prevention() {
    let commando = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ORDRUUN-COMMANDO")
        .expect("Ordruun Commando definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&commando.id));
    assert!(
        commando
            .supported_rules
            .contains(&"activated-prevent-one-damage-to-self")
    );
}
