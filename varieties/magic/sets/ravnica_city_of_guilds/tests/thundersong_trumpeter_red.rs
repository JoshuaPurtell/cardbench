use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn thundersong_trumpeter_is_executable_with_its_combat_restriction_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-THUNDERSONG-TRUMPETER")
        .expect("Thundersong Trumpeter exists");
    assert_eq!(definition.name, "Thundersong Trumpeter");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red, Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(1)));
    assert!(definition
        .supported_rules
        .contains(&"tap-prevent-target-combat"));
}
