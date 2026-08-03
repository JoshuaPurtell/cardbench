//! Red discovery contract for Agrus Kos's attack-triggered color-specific
//! combat modifiers.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn agrus_kos_requires_its_attack_triggered_color_specific_combat_modifiers() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AGRUS-KOS-WOJEK-VETERAN")
        .expect("Agrus Kos definition exists");

    assert_eq!(definition.name, "Agrus Kos, Wojek Veteran");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::White])
    );
    assert_eq!(definition.colors, BTreeSet::from([Color::Red, Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attack-triggered-color-specific-combat-modifiers")
    );

}
