//! Red coverage probe for Belltower Sphinx's shared Flying rule.
//!
//! The printed damage-triggered behavior remains intentionally outside this
//! bounded slice; this asks only for the static evasion keyword.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn belltower_sphinx_exposes_its_supported_flying_compatibility_slice() {
    let sphinx = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx definition exists");

    assert_eq!(sphinx.name, "Belltower Sphinx");
    assert_eq!(
        sphinx.mana_cost,
        ManaCost::with_colors(4, [Color::Blue])
    );
    assert_eq!(sphinx.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(sphinx.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sphinx.power, sphinx.toughness), (Some(2), Some(5)));
    assert_eq!(sphinx.keywords, [Keyword::Flying]);
    assert!(sphinx.effects.is_empty());
    assert_eq!(
        sphinx.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"],
        "the damage-triggered behavior remains intentionally bounded"
    );
}
