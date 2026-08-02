//! Full-fidelity public contract for Belltower Sphinx.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn belltower_sphinx_definition_includes_its_damage_trigger() {
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
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sphinx.id),
        "the represented damage trigger promotes this definition to full fidelity"
    );
}
