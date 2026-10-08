//! Candidate-owned RAV Zephyr Spirit definition and Blocks trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ZEPHYR-SPIRIT",
        name: "Zephyr Spirit",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "blocks-return-source-owner-hand",
        ],
        power: Some(0),
        toughness: Some(6),
        keywords: vec![],
        effects: vec![],
    }
}

/// Return this source to its owner's hand when it becomes a declared blocker.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-ZEPHYR-SPIRIT",
        ability: TriggeredAbility {
            id: "blocks-return-source-owner-hand",
            condition: TriggerCondition::Blocks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::ReturnSourceToOwnersHand],
        },
    }
}
