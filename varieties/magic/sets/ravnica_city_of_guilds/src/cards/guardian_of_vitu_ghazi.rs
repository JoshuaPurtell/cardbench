//! Candidate-owned RAV Guardian of Vitu-Ghazi definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// Printed cost, body, Convoke, and Vigilance characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GUARDIAN-OF-VITU-GHAZI",
        name: "Guardian of Vitu-Ghazi",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "vigilance",
        ],
        power: Some(4),
        toughness: Some(7),
        keywords: vec![Keyword::Convoke, Keyword::Vigilance],
        effects: vec![],
    }
}
