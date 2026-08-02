//! Regression for Benevolent Ancestor's shared Defender rule.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn benevolent_ancestor_exposes_its_defender_rule() {
    let ancestor = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BENEVOLENT-ANCESTOR")
        .expect("Benevolent Ancestor definition exists");

    assert_eq!(ancestor.name, "Benevolent Ancestor");
    assert_eq!(ancestor.mana_cost, ManaCost::with_colors(2, [Color::White]));
    assert_eq!(ancestor.colors, BTreeSet::from([Color::White]));
    assert_eq!(ancestor.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((ancestor.power, ancestor.toughness), (Some(0), Some(4)));
    assert_eq!(ancestor.keywords, [Keyword::Defender]);
    assert!(ancestor.effects.is_empty());
    assert_eq!(
        ancestor.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "tap-prevent-one-damage-to-target",
        ]
    );
}
