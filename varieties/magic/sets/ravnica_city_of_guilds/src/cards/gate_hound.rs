//! Candidate-owned definition and static effect for RAV Gate Hound.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Keyword, ManaCost,
    StaticContinuousEffectBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GATE-HOUND",
        name: "Gate Hound",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-controller-vigilance-while-enchanted",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn static_continuous_effect() -> StaticContinuousEffectBinding {
    StaticContinuousEffectBinding {
        card_definition: "RAV-GATE-HOUND",
        change: ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(
            Keyword::Vigilance,
        ),
    }
}
