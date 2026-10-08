//! Candidate-owned RAV Scion of the Wild definition and characteristic effect.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, ManaCost, StaticContinuousEffectBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SCION-OF-THE-WILD",
        name: "Scion of the Wild",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "dynamic-controlled-creature-count",
        ],
        power: Some(0),
        toughness: Some(0),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn static_continuous_effect() -> StaticContinuousEffectBinding {
    StaticContinuousEffectBinding {
        card_definition: "RAV-SCION-OF-THE-WILD",
        change: ContinuousChange::ControlledCreatureCountPowerToughness,
    }
}
