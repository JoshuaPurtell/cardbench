use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn boros_guildmage_is_executable_with_both_keyword_grant_abilities() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-GUILDMAGE")
        .expect("Boros Guildmage exists");
    assert_eq!(definition.name, "Boros Guildmage");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red, Color::White]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(definition
        .supported_rules
        .contains(&"activated-grant-haste"));
    assert!(definition
        .supported_rules
        .contains(&"activated-grant-first-strike"));
}
