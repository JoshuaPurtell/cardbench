//! Candidate implementation for the RAV Boros Recruit pilot.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, HybridManaSymbol, Keyword, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BOROS-RECRUIT",
        name: "Boros Recruit",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            0,
            [],
            [HybridManaSymbol {
                first: Color::Red,
                second: Color::White,
            }],
        ),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "hybrid-cost-casting", "first-strike"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::FirstStrike],
        effects: vec![],
    }
}
