//! Red RAV coverage probe for Selesnya Sagittars' shared Reach rule.
//!
//! The printed tap-to-damage activation remains deliberately outside this
//! bounded slice; this contract requests only the existing static Reach
//! substrate.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{card_definitions, RAV_FULL_FIDELITY_DEFINITION_IDS};

#[test]
fn selesnya_sagittars_exposes_its_reach_compatibility_slice() {
    let sagittars = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SELESNYA-SAGITTARS")
        .expect("Selesnya Sagittars definition exists");

    assert_eq!(sagittars.name, "Selesnya Sagittars");
    assert_eq!(
        sagittars.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::White])
    );
    assert_eq!(
        sagittars.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(sagittars.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((sagittars.power, sagittars.toughness), (Some(2), Some(5)));
    assert_eq!(sagittars.keywords, [Keyword::Reach]);
    assert!(sagittars.effects.is_empty());
    assert_eq!(
        sagittars.supported_rules,
        ["colored-cost-casting", "base-characteristics", "reach"],
        "the printed tap-to-damage activation remains intentionally bounded"
    );
    assert!(
        !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&sagittars.id),
        "the omitted activation keeps Selesnya Sagittars outside positive fidelity"
    );
}
