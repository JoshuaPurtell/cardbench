//! Red discovery contract for Breath of Fury's combat-trigger fidelity gap.

use cardbench_magic_rav::{card_definitions, executable_definition_id_for_collector};

#[test]
fn breath_of_fury_has_an_executable_attached_combat_definition() {
    assert_eq!(
        executable_definition_id_for_collector(116),
        Ok("RAV-BREATH-OF-FURY")
    );
    assert!(
        card_definitions()
            .into_iter()
            .any(|definition| definition.id == "RAV-BREATH-OF-FURY")
    );
}
