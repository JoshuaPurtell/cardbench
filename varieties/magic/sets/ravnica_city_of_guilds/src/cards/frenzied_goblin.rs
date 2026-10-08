//! Candidate-owned Frenzied Goblin definition and optional attack trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FRENZIED-GOBLIN",
        name: "Frenzied Goblin",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "attack-trigger-optional-red-cannot-block",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// When it attacks, its controller may pay red and choose a creature that
/// cannot block this turn.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FRENZIED-GOBLIN",
        ability: TriggeredAbility {
            id: "attack-cannot-block",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            optional: true,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::CannotBlock,
            }],
        },
    }
}
