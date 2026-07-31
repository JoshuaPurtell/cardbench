//! Red coverage probe for Carven Caryatid's shared Defender rule.
//!
//! Its enter-the-battlefield draw trigger remains intentionally outside this
//! bounded slice; this asks only for the static keyword already supported by
//! the expansion-neutral combat substrate.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn carven_caryatid_exposes_its_supported_defender_compatibility_slice() {
    let caryatid = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CARVEN-CARYATID")
        .expect("Carven Caryatid definition exists");

    assert_eq!(caryatid.name, "Carven Caryatid");
    assert_eq!(
        caryatid.mana_cost,
        ManaCost::with_colors(1, [Color::Green, Color::Green])
    );
    assert_eq!(caryatid.colors, BTreeSet::from([Color::Green]));
    assert_eq!(caryatid.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((caryatid.power, caryatid.toughness), (Some(2), Some(5)));
    assert_eq!(caryatid.keywords, [Keyword::Defender]);
    assert!(caryatid.effects.is_empty());
    assert_eq!(
        caryatid.supported_rules,
        ["colored-cost-casting", "base-characteristics", "defender"],
        "the enter-the-battlefield draw trigger remains intentionally bounded"
    );
}
