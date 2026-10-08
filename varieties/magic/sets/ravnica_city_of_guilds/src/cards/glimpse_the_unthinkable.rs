//! Candidate-owned RAV Glimpse the Unthinkable definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GLIMPSE-THE-UNTHINKABLE",
        name: "Glimpse the Unthinkable",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-mill-ten"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::MillTargetPlayer { count: 10 }],
    }
}
