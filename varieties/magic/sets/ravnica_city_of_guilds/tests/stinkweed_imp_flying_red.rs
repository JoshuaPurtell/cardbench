//! Red coverage probe for Stinkweed Imp's shared Flying rule.
//!
//! The independent full-fidelity trigger contract covers the combat-damage
//! behavior; this regression keeps the static evasion characteristics covered.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn stinkweed_imp_exposes_its_supported_flying_compatibility_slice() {
    let imp = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-STINKWEED-IMP")
        .expect("Stinkweed Imp definition exists");

    assert_eq!(imp.name, "Stinkweed Imp");
    assert_eq!(imp.mana_cost, ManaCost::with_colors(2, [Color::Black]));
    assert_eq!(imp.colors, BTreeSet::from([Color::Black]));
    assert_eq!(imp.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((imp.power, imp.toughness), (Some(1), Some(2)));
    assert_eq!(imp.keywords, [Keyword::Flying, Keyword::Dredge(5)]);
    assert!(imp.effects.is_empty());
    assert_eq!(
        imp.supported_rules,
        [
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "flying",
            "combat-damage-destroy-recipient",
        ]
    );
}
