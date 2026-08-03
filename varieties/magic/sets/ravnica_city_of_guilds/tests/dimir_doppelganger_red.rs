//! Red contract for Dimir Doppelganger's graveyard-copy activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn dimir_doppelganger_has_a_full_fidelity_graveyard_copy_definition() {
    assert_eq!(
        executable_definition_id_for_collector(202),
        Ok("RAV-DIMIR-DOPPELGANGER")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-DOPPELGANGER")
        .expect("Dimir Doppelganger definition exists");
    assert_eq!(definition.name, "Dimir Doppelganger");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Black])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Blue, Color::Black])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(definition.power, Some(0));
    assert_eq!(definition.toughness, Some(2));
    assert!(definition
        .supported_rules
        .contains(&"exile-target-creature-card-from-any-graveyard-copy-source"));
    assert!(definition
        .supported_rules
        .contains(&"copied-form-retains-graveyard-copy-activation"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
