//! Candidate-owned RAV Compulsive Research definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-COMPULSIVE-RESEARCH",
        name: "Compulsive Research",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "target-player-draw-three-conditional-private-discard",
            "policy-submitted-private-discard",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DrawTargetPlayerThenConditionalPrivateDiscard],
    }
}
