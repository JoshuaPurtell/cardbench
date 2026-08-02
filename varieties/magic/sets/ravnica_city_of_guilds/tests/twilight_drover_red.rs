//! Red discovery contract for Twilight Drover's leave-the-battlefield trigger
//! and flying Spirit activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn twilight_drover_definition_and_rules_contract_exist() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TWILIGHT-DROVER")
        .expect("Twilight Drover definition exists");
    assert_eq!(definition.name, "Twilight Drover");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"another-creature-leaves-battlefield-plus-one-counter")
    );
    assert!(
        definition
            .supported_rules
            .contains(&"activated-create-flying-spirit")
    );
}
