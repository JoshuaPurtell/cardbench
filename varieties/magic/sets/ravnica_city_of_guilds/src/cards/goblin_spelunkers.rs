//! CardBench-authored RAV Goblin Spelunkers identity and keyword binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed characteristics for the RAV red creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOBLIN-SPELUNKERS",
        name: "Goblin Spelunkers",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "mountainwalk",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Mountainwalk],
        effects: vec![],
    }
}
