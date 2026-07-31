use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn sabertooth_alley_cat_is_executable_with_its_mountain_block_condition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SABERTOOTH-ALLEY-CAT")
        .expect("Sabertooth Alley Cat exists");
    assert_eq!(definition.name, "Sabertooth Alley Cat");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(1)));
    assert!(definition
        .supported_rules
        .contains(&"mountain-required-to-block"));
}
