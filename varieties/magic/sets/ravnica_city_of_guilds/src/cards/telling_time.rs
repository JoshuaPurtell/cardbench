//! Candidate-owned RAV Telling Time definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TELLING-TIME",
        name: "Telling Time",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "private-top-library-hand-top-bottom-partition",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 3 }],
    }
}
