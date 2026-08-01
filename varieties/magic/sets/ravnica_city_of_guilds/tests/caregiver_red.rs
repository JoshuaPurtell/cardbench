//! Red discovery contract for Caregiver's targeted prevention activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn caregiver_requires_its_sacrifice_cost_targeted_prevention_activation() {
    let caregiver = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CAREGIVER")
        .expect("Caregiver definition exists");

    assert_eq!(caregiver.name, "Caregiver");
    assert_eq!(caregiver.mana_cost, ManaCost::with_colors(0, [Color::White]));
    assert_eq!(caregiver.colors, BTreeSet::from([Color::White]));
    assert_eq!(caregiver.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((caregiver.power, caregiver.toughness), (Some(1), Some(1)));
    assert!(caregiver.keywords.is_empty());
    assert_eq!(caregiver.effects, []);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&caregiver.id));
    assert!(
        caregiver
            .supported_rules
            .contains(&"sacrifice-source-targeted-one-damage-prevention")
    );
}
