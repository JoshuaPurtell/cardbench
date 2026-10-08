//! Candidate-owned RAV Infectious Host definition and dies-trigger binding.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-INFECTIOUS-HOST",
        name: "Infectious Host",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "dies-target-player-life-loss",
            "policy-selected-trigger-target",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// Infectious Host's mandatory dies ability targets a player who loses 2 life.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-INFECTIOUS-HOST",
        ability: TriggeredAbility {
            id: "dies-target-player-life-loss",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Player],
            effects: vec![Effect::LoseLifeTarget { amount: 2 }],
        },
    }
}
