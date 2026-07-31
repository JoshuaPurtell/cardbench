//! Red coverage probe for Goblin Fire Fiend's shared Haste rule.
//!
//! Its full behavior is asserted separately; this probe retains only the
//! reusable static Haste contract.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn goblin_fire_fiend_exposes_its_bounded_haste_slice() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");

    assert_eq!(fiend.name, "Goblin Fire Fiend");
    assert_eq!(fiend.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(fiend.colors, BTreeSet::from([Color::Red]));
    assert_eq!(fiend.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((fiend.power, fiend.toughness), (Some(1), Some(1)));
    assert!(fiend.keywords.contains(&Keyword::Haste));
    assert!(fiend.effects.is_empty());
    assert_eq!(
        fiend.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "haste",
            "must-block-if-able",
            "activated-plus-one-power",
        ]
    );
}
