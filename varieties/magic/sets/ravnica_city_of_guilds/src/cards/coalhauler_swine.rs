//! Candidate-owned definition and damage trigger for Coalhauler Swine.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-COALHAULER-SWINE",
        name: "Coalhauler Swine",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-received-to-each-player",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-COALHAULER-SWINE",
        ability: TriggeredAbility {
            id: "damage-each-player",
            condition: TriggerCondition::ReceivesDamage,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DealDamageToEachPlayerFromReceivedDamage],
        },
    }
}
