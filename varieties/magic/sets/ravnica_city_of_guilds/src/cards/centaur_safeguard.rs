//! Candidate implementation for the RAV Centaur Safeguard diagnostic pilot.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, HybridManaSymbol, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CENTAUR-SAFEGUARD",
        name: "Centaur Safeguard",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
            ],
        ),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "hybrid-cost-casting",
            "base-characteristics",
            "optional-dies-gain-three-life",
        ],
        power: Some(3),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// The optional ability created when Centaur Safeguard dies.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-CENTAUR-SAFEGUARD",
        ability: TriggeredAbility {
            id: "dies-may-gain-three-life",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 3 }],
        },
    }
}
