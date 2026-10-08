//! Candidate-owned definition for the RAV Greater Mossdog.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GREATER-MOSSDOG",
        name: "Greater Mossdog",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "dredge", "base-characteristics"],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![Keyword::Dredge(3)],
        effects: vec![],
    }
}
