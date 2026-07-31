//! Red coverage probe for Tattered Drake's shared Flying rule.
//!
//! Its regeneration activation is deliberately not approximated here.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn tattered_drake_exposes_its_supported_flying_compatibility_slice() {
    let drake = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TATTERED-DRAKE")
        .expect("Tattered Drake definition exists");

    assert_eq!(drake.name, "Tattered Drake");
    assert_eq!(drake.mana_cost, ManaCost::with_colors(4, [Color::Blue]));
    assert_eq!(drake.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(drake.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((drake.power, drake.toughness), (Some(2), Some(2)));
    assert_eq!(drake.keywords, [Keyword::Flying]);
    assert!(drake.effects.is_empty());
    assert_eq!(
        drake.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"],
        "the regeneration activation remains intentionally bounded"
    );
}
