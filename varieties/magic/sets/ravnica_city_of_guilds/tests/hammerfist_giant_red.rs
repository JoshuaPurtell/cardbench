//! Red discovery probe for Hammerfist Giant's tap damage ability.

use cardbench_magic_engine::{CardType, Keyword};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hammerfist_giant_requires_nonflying_global_damage_and_tap_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HAMMERFIST-GIANT")
        .expect("Hammerfist Giant definition exists");
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"tap-global-nonflying-damage")
    );
    assert_eq!(definition.power, Some(5));
    assert_eq!(definition.toughness, Some(4));
    assert!(!definition.keywords.contains(&Keyword::Flying));
}
