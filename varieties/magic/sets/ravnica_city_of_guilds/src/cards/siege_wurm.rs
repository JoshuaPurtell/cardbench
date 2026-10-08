//! Candidate-owned definition for the RAV Siege Wurm.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SIEGE-WURM",
        name: "Siege Wurm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "trample",
        ],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![Keyword::Convoke, Keyword::Trample],
        effects: vec![],
    }
}
