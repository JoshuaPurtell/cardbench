//! Red coverage probe for Grozoth's already-supported shared mechanics.
//!
//! Grozoth also has an entry trigger which remains outside this compatibility
//! slice.  This probe is deliberately limited to Defender and the existing
//! immediate hand-zone Transmute operation, both of which the shared engine
//! can already represent.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn grozoth_exposes_defender_and_bounded_transmute_compatibility() {
    let grozoth = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GROZOTH")
        .expect("Grozoth definition exists");

    assert_eq!(grozoth.name, "Grozoth");
    assert_eq!(
        grozoth.mana_cost,
        ManaCost::with_colors(6, [Color::Blue; 3])
    );
    assert_eq!(grozoth.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(grozoth.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((grozoth.power, grozoth.toughness), (Some(9), Some(9)));
    assert_eq!(
        grozoth.keywords,
        [
            Keyword::Defender,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
        ],
        "the shared engine already supports these printed mechanics"
    );
    assert_eq!(grozoth.effects, Vec::new());
    assert_eq!(
        grozoth.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "immediate-hand-zone-transmute-compatibility",
        ],
        "the entry trigger and stack-backed activation remain explicitly outside this slice"
    );
}
