//! Candidate-owned RAV Mountain definition and typed basic-land binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, Color, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MOUNTAIN",
        name: "Mountain",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Red]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &[
            "full-rules-fidelity",
            "basic-land-type-line",
            "intrinsic-single-color-mana-ability",
            "basic-land-deck-construction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn basic_land_type_binding() -> BasicLandTypeBinding {
    BasicLandTypeBinding {
        card_definition: "RAV-MOUNTAIN",
        land_type: BasicLandType::Mountain,
    }
}
