//! CardBench-authored RAV Gather Courage definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed identity and executable Convoke pump effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GATHER-COURAGE",
        name: "Gather Courage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "targeted-layer-7-modifier",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: 2,
            toughness: 2,
        }],
    }
}
