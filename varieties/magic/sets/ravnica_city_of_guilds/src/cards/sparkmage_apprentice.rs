//! CardBench-authored RAV Sparkmage Apprentice definition and ETB trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

/// Printed identity and base characteristics.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SPARKMAGE-APPRENTICE",
        name: "Sparkmage Apprentice",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "etb-targeted-damage",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// The enters-the-battlefield trigger deals one damage to a target player or creature.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SPARKMAGE-APPRENTICE",
        ability: TriggeredAbility {
            id: "etb-deal-one",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::PlayerOrCreature],
            effects: vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::PlayerOrCreature,
            }],
        },
    }
}
