//! Candidate-owned RAV Dimir Infiltrator definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// RAV #203 is an unblockable blue-black 1/3 Spirit with Transmute `{1}{U}{B}`.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DIMIR-INFILTRATOR",
        name: "Dimir Infiltrator",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "unblockable",
            "stack-backed-private-transmute",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![
            Keyword::Unblockable,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Black])),
        ],
        effects: vec![],
    }
}
