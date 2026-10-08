//! CardBench-authored RAV Courier Hawk definition, isolated for trace qualification.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed characteristics for the RAV white creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-COURIER-HAWK",
        name: "Courier Hawk",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "vigilance",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![Keyword::Flying, Keyword::Vigilance],
        effects: vec![],
    }
}
