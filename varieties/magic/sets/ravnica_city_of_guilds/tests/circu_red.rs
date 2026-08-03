//! Red contract for Circu, Dimir Lobotomist's colored-spell trigger boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn circu_has_an_executable_full_fidelity_definition() {
    assert_eq!(
        executable_definition_id_for_collector(196),
        Ok("RAV-CIRCU-DIMIR-LOBOTOMIST")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CIRCU-DIMIR-LOBOTOMIST")
        .expect("Circu definition exists");
    assert_eq!(definition.name, "Circu, Dimir Lobotomist");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Blue, Color::Black])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(2));
    assert_eq!(definition.toughness, Some(3));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
