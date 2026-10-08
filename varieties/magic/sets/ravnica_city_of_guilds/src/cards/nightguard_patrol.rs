//! Candidate implementation for the RAV Nightguard Patrol diagnostic pilot.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-NIGHTGUARD-PATROL",
        name: "Nightguard Patrol",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "first-strike",
            "vigilance",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![Keyword::FirstStrike, Keyword::Vigilance],
        effects: vec![],
    }
}
