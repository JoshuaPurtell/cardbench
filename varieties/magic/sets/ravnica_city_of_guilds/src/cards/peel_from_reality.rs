use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

use crate::{SET_CODE, colors, types};

/// Candidate-owned definition for Peel from Reality.
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PEEL-FROM-REALITY",
        name: "Peel from Reality",
        set_code: SET_CODE,
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: colors([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "controlled-creature-target",
            "opponent-creature-target",
            "owner-preserving-bounce",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ReturnControlledCreatureToHand,
            Effect::ReturnOpponentCreatureToHand,
        ],
    }
}
