//! Candidate-owned definition and triggered ability for RAV Searing Meditation.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Searing Meditation is a red-white enchantment costing `{2}{R}{W}`.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SEARING-MEDITATION",
        name: "Searing Meditation",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "life-gain-trigger",
            "optional-colored-trigger-cost",
            "triggered-two-damage-target",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Whenever its controller gains life, the controller may pay `{1}{R}{W}`.
/// If they do, the trigger deals 2 damage to any target.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SEARING-MEDITATION",
        ability: TriggeredAbility {
            id: "life-gain-deal-two",
            condition: TriggerCondition::LifeGained,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            optional: true,
            targets: vec![],
            effects: vec![Effect::DealDamageAfterOptionalManaPayment {
                amount: 2,
                target: TargetRequirement::PlayerOrCreature,
            }],
        },
    }
}
