//! Candidate-owned RAV Moroii definition and upkeep trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MOROII",
        name: "Moroii",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "upkeep-controller-life-loss",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-MOROII",
        ability: TriggeredAbility {
            id: "upkeep-lose-one-life",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::LoseLifeController { amount: 1 }],
        },
    }
}
