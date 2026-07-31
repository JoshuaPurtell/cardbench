use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::card_definitions;

#[test]
fn war_torch_goblin_is_executable_with_its_sacrifice_damage_ability() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WAR-TORCH-GOBLIN")
        .expect("War-Torch Goblin exists");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert!(definition.keywords.is_empty());
    assert!(definition.supported_rules.contains(&"sacrifice-source"));
}
