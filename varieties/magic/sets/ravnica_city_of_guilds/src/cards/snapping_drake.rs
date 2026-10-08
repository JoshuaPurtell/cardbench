//! CardBench-authored Snapping Drake definition, isolated for trace qualification.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Complete currently represented Snapping Drake functionality.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SNAPPING-DRAKE",
        name: "Snapping Drake",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
        ],
        power: Some(3),
        toughness: Some(2),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}
