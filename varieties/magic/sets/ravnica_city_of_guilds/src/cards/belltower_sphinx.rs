//! CardBench-authored RAV Belltower Sphinx definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

/// Printed characteristics and damage-triggered mill mechanic.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BELLTOWER-SPHINX",
        name: "Belltower Sphinx",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "damage-received-source-controller-mill-that-many",
        ],
        power: Some(2),
        toughness: Some(5),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// Mandatory non-targeted damage trigger that mills the damage source's controller.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-BELLTOWER-SPHINX",
        ability: TriggeredAbility {
            id: "damage-source-controller-mill-that-many",
            condition: TriggerCondition::ReceivesDamage,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::MillSourceControllerFromSourceDamage],
        },
    }
}
