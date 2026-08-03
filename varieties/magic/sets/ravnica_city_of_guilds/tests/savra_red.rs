//! Red contract for Savra's sacrifice-color trigger boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn savra_has_an_executable_full_fidelity_definition() {
    assert_eq!(
        executable_definition_id_for_collector(225),
        Ok("RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SAVRA-QUEEN-OF-THE-GOLGARI")
        .expect("Savra definition exists");
    assert_eq!(definition.name, "Savra, Queen of the Golgari");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(2));
    assert_eq!(definition.toughness, Some(2));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
