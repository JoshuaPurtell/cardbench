//! Candidate-owned RAV Oathsworn Giant definition and static effects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Keyword, ManaCost,
    StaticContinuousEffectBinding,
};

/// Printed characteristics of Oathsworn Giant.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-OATHSWORN-GIANT",
        name: "Oathsworn Giant",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "vigilance",
            "static-other-creatures-vigilance-plus-zero-two",
        ],
        power: Some(3),
        toughness: Some(4),
        keywords: vec![Keyword::Vigilance],
        effects: vec![],
    }
}

/// Other creatures under the Giant's controller get +0/+2 and vigilance.
#[must_use]
pub(crate) fn static_continuous_effects() -> [StaticContinuousEffectBinding; 2] {
    [
        StaticContinuousEffectBinding {
            card_definition: "RAV-OATHSWORN-GIANT",
            change: ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                power: 0,
                toughness: 2,
            },
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-OATHSWORN-GIANT",
            change: ContinuousChange::OtherControlledCreaturesAddKeyword(Keyword::Vigilance),
        },
    ]
}
