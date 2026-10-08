//! Candidate-owned RAV Conclave Equenaut definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed identity and executable Convoke/Flying characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CONCLAVE-EQUENAUT",
        name: "Conclave Equenaut",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "flying",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![Keyword::Convoke, Keyword::Flying],
        effects: vec![],
    }
}
