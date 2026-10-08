//! Candidate-owned RAV Last Gasp spell definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LAST-GASP",
        name: "Last Gasp",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -3,
            toughness: -3,
        }],
    }
}
