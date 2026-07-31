use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn wojek_embermage_is_executable_with_its_radiance_tap_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOJEK-EMBERMAGE")
        .expect("Wojek Embermage exists");
    assert_eq!(definition.name, "Wojek Embermage");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(1), Some(1)));
    assert!(definition
        .supported_rules
        .contains(&"tap-radiance-one-damage"));
}
