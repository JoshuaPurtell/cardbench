//! Red coverage probe for Divebomber Griffin's shared Flying rule.
//!
//! Its printed activated sacrifice/damage ability remains intentionally
//! outside this bounded slice.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn divebomber_griffin_exposes_its_supported_flying_compatibility_slice() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIVEBOMBER-GRIFFIN")
        .expect("Divebomber Griffin definition exists");

    assert_eq!(griffin.name, "Divebomber Griffin");
    assert_eq!(
        griffin.mana_cost,
        ManaCost::with_colors(3, [Color::White, Color::White])
    );
    assert_eq!(griffin.colors, BTreeSet::from([Color::White]));
    assert_eq!(griffin.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((griffin.power, griffin.toughness), (Some(3), Some(2)));
    assert_eq!(griffin.keywords, [Keyword::Flying]);
    assert!(griffin.effects.is_empty());
    assert_eq!(
        griffin.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"],
        "the activated sacrifice/damage ability remains intentionally bounded"
    );
}
