//! CardBench-authored RAV Goliath Spider identity and Reach binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed characteristics for the RAV green creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLIATH-SPIDER",
        name: "Goliath Spider",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "reach",
        ],
        power: Some(7),
        toughness: Some(6),
        keywords: vec![Keyword::Reach],
        effects: vec![],
    }
}
