//! Candidate-owned RAV Keening Banshee definition and enters trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-KEENING-BANSHEE",
        name: "Keening Banshee",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "enter-battlefield-targeted-minus-two-minus-two",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// The mandatory ETB trigger gives its target creature -2/-2 until end of turn.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-KEENING-BANSHEE",
        ability: TriggeredAbility {
            id: "etb-target-minus-two",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -2,
                toughness: -2,
            }],
        },
    }
}
