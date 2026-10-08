//! Candidate-owned definition and mandatory team-pump ETB binding for RAV Flame-Kin Zealot.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FLAME-KIN-ZEALOT",
        name: "Flame-Kin Zealot",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red, Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-team-pump-haste",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FLAME-KIN-ZEALOT",
        ability: TriggeredAbility {
            id: "etb-team-pump-haste",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                },
                Effect::AddKeywordToControllerCreaturesUntilEndOfTurn {
                    keyword: Keyword::Haste,
                },
            ],
        },
    }
}
