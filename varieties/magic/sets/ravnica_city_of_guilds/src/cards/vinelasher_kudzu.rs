//! Candidate-owned RAV Vinelasher Kudzu identity and landfall ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VINELASHER-KUDZU",
        name: "Vinelasher Kudzu",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controlled-land-entry-plus-one-counter",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// Whenever a land enters under your control, put a +1/+1 counter here.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-VINELASHER-KUDZU",
        ability: TriggeredAbility {
            id: "controller-landfall-plus-one-counter",
            condition: TriggerCondition::ControlledLandEntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    }
}
