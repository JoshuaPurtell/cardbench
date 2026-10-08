use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

use crate::{SET_CODE, colors, types};

/// Candidate-owned definition for Sabertooth Alley Cat.
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SABERTOOTH-ALLEY-CAT",
        name: "Sabertooth Alley Cat",
        set_code: SET_CODE,
        mana_cost: ManaCost::with_colors(1, [Color::Red, Color::Red]),
        colors: colors([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "mountain-required-to-block",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![Keyword::CannotBlockUnlessControlsMountain],
        effects: vec![],
    }
}
