//! Candidate implementation for the RAV Boros Swiftblade pilot.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BOROS-SWIFTBLADE",
        name: "Boros Swiftblade",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "double-strike",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![Keyword::DoubleStrike],
        effects: vec![],
    }
}
