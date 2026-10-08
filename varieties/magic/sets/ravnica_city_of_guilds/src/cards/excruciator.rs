//! Candidate-owned definition for the RAV Excruciator.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-EXCRUCIATOR",
        name: "Excruciator",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-cannot-be-prevented",
        ],
        power: Some(7),
        toughness: Some(7),
        keywords: vec![Keyword::DamageCannotBePrevented],
        effects: vec![],
    }
}
