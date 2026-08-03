//! Red contract for Razia's Purification's executable preservation boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn razias_purification_has_an_executable_full_fidelity_definition() {
    assert_eq!(
        executable_definition_id_for_collector(224),
        Ok("RAV-RAZIAS-PURIFICATION")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RAZIAS-PURIFICATION")
        .expect("Razia's Purification definition exists");
    assert_eq!(definition.name, "Razia's Purification");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Red, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
