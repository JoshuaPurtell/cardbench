//! Candidate implementation for the RAV Dromad Purebred diagnostic pilot.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DROMAD-PUREBRED",
        name: "Dromad Purebred",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "damage-received-life-gain",
        ],
        power: Some(1),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

/// One nonoptional life-gain trigger for each positive damage receipt.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-DROMAD-PUREBRED",
        ability: TriggeredAbility {
            id: "damage-gain-life-from-source-damage",
            condition: TriggerCondition::ReceivesDamage,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeControllerFromSourceDamage],
        },
    }
}
