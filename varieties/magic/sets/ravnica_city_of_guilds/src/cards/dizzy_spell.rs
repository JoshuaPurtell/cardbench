//! Candidate-owned RAV Dizzy Spell definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed instant characteristics, layer-seven effect, and hand-zone Transmute cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DIZZY-SPELL",
        name: "Dizzy Spell",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "targeted-layer-7-modifier",
            "transmute",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -3,
            toughness: 0,
        }],
    }
}
