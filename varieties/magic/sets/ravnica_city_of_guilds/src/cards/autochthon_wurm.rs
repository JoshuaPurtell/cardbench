//! Candidate-owned definition for RAV Autochthon Wurm.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-AUTOCHTHON-WURM",
        name: "Autochthon Wurm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(
            10,
            [
                Color::Green,
                Color::Green,
                Color::Green,
                Color::White,
                Color::White,
            ],
        ),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "trample",
            "multi-block-trample-combat-damage",
        ],
        power: Some(9),
        toughness: Some(14),
        keywords: vec![Keyword::Convoke, Keyword::Trample],
        effects: vec![],
    }
}
