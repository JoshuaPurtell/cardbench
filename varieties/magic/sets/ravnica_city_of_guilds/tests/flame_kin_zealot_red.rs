use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn flame_kin_zealot_is_executable_with_its_enter_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLAME-KIN-ZEALOT")
        .expect("Flame-Kin Zealot exists");
    assert_eq!(definition.name, "Flame-Kin Zealot");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red, Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(definition
        .supported_rules
        .contains(&"etb-team-pump-haste"));
}
