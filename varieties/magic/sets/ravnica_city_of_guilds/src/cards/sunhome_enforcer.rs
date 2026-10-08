//! Candidate implementation for the RAV Sunhome Enforcer diagnostic pilot.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SUNHOME-ENFORCER",
        name: "Sunhome Enforcer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-trigger-life-gain",
        ],
        power: Some(2),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

/// Life gain is materialized from this source's actual committed damage.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SUNHOME-ENFORCER",
        ability: TriggeredAbility {
            id: "damage-life-gain",
            condition: TriggerCondition::DealsDamage,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeControllerFromSourceDamage],
        },
    }
}
