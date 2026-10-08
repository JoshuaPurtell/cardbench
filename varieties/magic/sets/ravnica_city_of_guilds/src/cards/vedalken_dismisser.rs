//! Candidate-owned RAV Vedalken Dismisser definition and enters trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VEDALKEN-DISMISSER",
        name: "Vedalken Dismisser",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-target-creature-owner-library-top",
            "policy-submitted-trigger-target",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// The mandatory enters-the-battlefield trigger puts its target on its owner's library top.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-VEDALKEN-DISMISSER",
        ability: TriggeredAbility {
            id: "etb-target-creature-owner-library-top",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::PutTargetCreatureOnOwnersLibraryTop],
        },
    }
}
