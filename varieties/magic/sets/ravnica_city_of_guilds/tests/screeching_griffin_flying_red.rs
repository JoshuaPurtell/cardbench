//! Red coverage probe for Screeching Griffin's shared Flying rule.
//!
//! The printed activated combat ability remains intentionally outside this
//! bounded slice; this asks only for the already supported evasion keyword.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn screeching_griffin_exposes_its_supported_flying_compatibility_slice() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");

    assert_eq!(griffin.name, "Screeching Griffin");
    assert_eq!(griffin.mana_cost, ManaCost::with_colors(3, [Color::White]));
    assert_eq!(griffin.colors, BTreeSet::from([Color::White]));
    assert_eq!(griffin.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((griffin.power, griffin.toughness), (Some(2), Some(2)));
    assert_eq!(griffin.keywords, [Keyword::Flying]);
    assert!(griffin.effects.is_empty());
    assert_eq!(
        griffin.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "activated-prevent-target-blocking-source"
        ]
    );
}
