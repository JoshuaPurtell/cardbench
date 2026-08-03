//! Compatibility contract retained after Belltower Sphinx's full promotion.

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
    assert_eq!(sphinx.mana_cost, ManaCost::with_colors(4, [Color::Blue]));
    assert_eq!(sphinx.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(sphinx.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sphinx.power, sphinx.toughness), (Some(2), Some(5)));
    assert_eq!(sphinx.keywords, [Keyword::Flying]);
    assert!(sphinx.effects.is_empty());
    assert_eq!(
        sphinx.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "damage-received-source-controller-mill-that-many",
        ],
        "the promoted damage-triggered behavior remains explicit"
    );
}
