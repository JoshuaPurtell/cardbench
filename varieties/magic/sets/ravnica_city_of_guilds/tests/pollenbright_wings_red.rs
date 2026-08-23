//! Red discovery contract for Pollenbright Wings' attached-combat trigger.

use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn pollenbright_wings_requires_an_executable_attached_combat_damage_definition() {
    assert_eq!(
        executable_definition_id_for_collector(219),
        Ok("RAV-POLLENBRIGHT-WINGS"),
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-POLLENBRIGHT-WINGS")
        .expect("Pollenbright Wings definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attached-creature-combat-damage-saproling-count")
    );
}
