//! Candidate-owned RAV Trophy Hunter definition and source-bound abilities.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

/// Printed identity and base characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TROPHY-HUNTER",
        name: "Trophy Hunter",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-flying-targeted-one-damage",
            "source-dealt-damage-delayed-death-trigger",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// Tapping and paying `{1}{G}` deals one damage to a flying creature.
#[must_use]
pub(crate) fn activated_ability_binding() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TROPHY-HUNTER",
        ability: ActivatedAbility {
            id: "deal-one-damage-to-flying-creature",
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::FlyingCreature],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::FlyingCreature,
            }],
        },
    }
}

/// A creature damaged by this exact battlefield incarnation dying this turn
/// queues the ordinary triggered ability, which adds a counter to the source.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-TROPHY-HUNTER",
        ability: TriggeredAbility {
            id: "damaged-creature-dies-add-plus-one-counter",
            condition: TriggerCondition::DamagedCreatureBySourceDiesThisTurn,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    }
}
