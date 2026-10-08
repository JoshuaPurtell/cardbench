//! Candidate implementation for the RAV Transluminant diagnostic pilot.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TRANSLUMINANT",
        name: "Transluminant",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "dies-create-flying-spirit",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// The optional Spirit token ability created when Transluminant dies.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-TRANSLUMINANT",
        ability: TriggeredAbility {
            id: "dies-create-flying-spirit",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::white_spirit(),
                count: 1,
            }],
        },
    }
}
