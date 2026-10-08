//! Candidate implementation for the RAV Skyknight Legionnaire pilot.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SKYKNIGHT-LEGIONNAIRE",
        name: "Skyknight Legionnaire",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "haste",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Flying, Keyword::Haste],
        effects: vec![],
    }
}
