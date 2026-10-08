//! Candidate-owned Golgari Brownscale definition and graveyard-to-hand trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #166's green 2/3 with Dredge 2 and a two-life graveyard return trigger.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-BROWNSCALE",
        name: "Golgari Brownscale",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "graveyard-to-hand-gain-life",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![Keyword::Dredge(2)],
        effects: vec![],
    }
}

/// Gains two life whenever this object returns from its owner's graveyard to hand.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GOLGARI-BROWNSCALE",
        ability: TriggeredAbility {
            id: "graveyard-to-hand-gain-life",
            condition: TriggerCondition::GraveyardToHand,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 2 }],
        },
    }
}
