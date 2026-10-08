//! Candidate-owned RAV Scatter the Seeds definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TokenSpec,
};

/// Printed identity and executable Convoke/Saproling token behavior.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SCATTER-THE-SEEDS",
        name: "Scatter the Seeds",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "convoke", "token-creation"],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 3,
        }],
    }
}
