//! Red discovery contract for Flash Conscription's temporary control slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn flash_conscription_requires_temporary_control_untap_and_haste() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLASH-CONSCRIPTION")
        .expect("Flash Conscription definition exists");

    assert_eq!(definition.name, "Flash Conscription");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"gain-control-until-eot")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"untap-target-permanent")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"grant-haste-until-eot")
    );
}
