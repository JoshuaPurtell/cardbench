//! Red contract for Warp World's public full-fidelity boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn warp_world_has_an_executable_full_fidelity_definition() {
    assert_eq!(
        executable_definition_id_for_collector(150),
        Ok("RAV-WARP-WORLD")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WARP-WORLD")
        .expect("Warp World definition exists");
    assert_eq!(definition.name, "Warp World");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Red, Color::Red, Color::Red])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Sorcery]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
