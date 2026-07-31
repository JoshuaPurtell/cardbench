//! Red coverage probe for Woebringer Demon's shared Flying rule.
//!
//! The upkeep sacrifice behavior remains deliberately outside this bounded
//! slice; this probe requests only the static evasion keyword.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::card_definitions;

#[test]
fn woebringer_demon_exposes_its_supported_flying_compatibility_slice() {
    let demon = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon definition exists");

    assert_eq!(demon.name, "Woebringer Demon");
    assert_eq!(
        demon.mana_cost,
        ManaCost::with_colors(3, [Color::Black, Color::Black])
    );
    assert_eq!(demon.colors, BTreeSet::from([Color::Black]));
    assert_eq!(demon.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((demon.power, demon.toughness), (Some(4), Some(4)));
    assert_eq!(demon.keywords, [Keyword::Flying]);
    assert!(demon.effects.is_empty());
    assert_eq!(
        demon.supported_rules,
        ["colored-cost-casting", "base-characteristics", "flying"],
        "the upkeep sacrifice behavior remains intentionally bounded"
    );
}
