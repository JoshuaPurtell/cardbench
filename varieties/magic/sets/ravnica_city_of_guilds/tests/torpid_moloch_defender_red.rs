//! Red coverage probe for Torpid Moloch's shared Defender rule.
//!
//! Its land-sacrifice activation is intentionally not approximated here. This
//! probe only requests the already supported static keyword.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn torpid_moloch_exposes_its_supported_defender_compatibility_slice() {
    let moloch = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TORPID-MOLOCH")
        .expect("Torpid Moloch definition exists");

    assert_eq!(moloch.name, "Torpid Moloch");
    assert_eq!(moloch.mana_cost, ManaCost::with_colors(0, [Color::Red]));
    assert_eq!(moloch.colors, BTreeSet::from([Color::Red]));
    assert_eq!(moloch.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((moloch.power, moloch.toughness), (Some(3), Some(2)));
    assert_eq!(moloch.keywords, [Keyword::Defender]);
    assert!(moloch.effects.is_empty());
    assert_eq!(
        moloch.supported_rules,
        ["colored-cost-casting", "base-characteristics", "defender"],
        "the card-specific land-sacrifice activation remains intentionally bounded"
    );
}
