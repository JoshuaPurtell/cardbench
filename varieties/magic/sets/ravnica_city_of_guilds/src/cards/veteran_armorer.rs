//! Candidate-owned definition and continuous-effect binding for RAV Veteran Armorer.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, ManaCost, StaticContinuousEffectBinding,
};

/// Printed characteristics of Veteran Armorer.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VETERAN-ARMORER",
        name: "Veteran Armorer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-other-creatures-plus-zero-one",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Other creatures under the Armorer's controller get +0/+1.
#[must_use]
pub(crate) fn static_continuous_effect() -> StaticContinuousEffectBinding {
    StaticContinuousEffectBinding {
        card_definition: "RAV-VETERAN-ARMORER",
        change: ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
            power: 0,
            toughness: 1,
        },
    }
}
