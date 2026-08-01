//! Red discovery contract for Blazing Archon's static attack restriction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn blazing_archon_requires_its_complete_static_attack_restriction() {
    let archon = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BLAZING-ARCHON")
        .expect("Blazing Archon definition exists");

    assert_eq!(archon.name, "Blazing Archon");
    assert_eq!(
        archon.mana_cost,
        ManaCost::with_colors(6, [Color::White, Color::White, Color::White])
    );
    assert_eq!(archon.colors, BTreeSet::from([Color::White]));
    assert_eq!(archon.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((archon.power, archon.toughness), (Some(5), Some(6)));
    assert_eq!(archon.keywords, [Keyword::Flying]);
    assert_eq!(archon.effects, []);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&archon.id));
    assert!(
        archon
            .supported_rules
            .contains(&"static-opponents-cannot-attack-controller")
    );
}
