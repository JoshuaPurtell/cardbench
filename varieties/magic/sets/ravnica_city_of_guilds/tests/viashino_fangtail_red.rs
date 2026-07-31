use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn viashino_fangtail_is_executable_with_its_tap_damage_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VIASHINO-FANGTAIL")
        .expect("Viashino Fangtail exists");
    assert_eq!(definition.name, "Viashino Fangtail");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(3)));
    assert!(definition
        .supported_rules
        .contains(&"tap-deal-one-to-player-or-creature"));
}
