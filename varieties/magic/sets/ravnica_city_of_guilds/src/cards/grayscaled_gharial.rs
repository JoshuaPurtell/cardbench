//! CardBench-authored RAV Grayscaled Gharial identity and Islandwalk binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{BasicLandType, CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed characteristics for the RAV blue creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GRAYSCALED-GHARIAL",
        name: "Grayscaled Gharial",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "islandwalk",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Landwalk(BasicLandType::Island)],
        effects: vec![],
    }
}
