//! Candidate-owned RAV Flow of Ideas definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{BasicLandType, CardDefinition, CardType, Color, Effect, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FLOW-OF-IDEAS",
        name: "Flow of Ideas",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "draw-for-each-controlled-island"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DrawControllerForEachControlledBasicLandType {
            land_type: BasicLandType::Island,
        }],
    }
}
