//! Candidate-owned RAV Woebringer Demon definition and upkeep trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WOEBRINGER-DEMON",
        name: "Woebringer Demon",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "each-upkeep-active-player-sacrifice-creature",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// Each upkeep player chooses a creature they control to sacrifice.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-WOEBRINGER-DEMON",
        ability: TriggeredAbility {
            id: "each-upkeep-active-player-sacrifice-creature",
            condition: TriggerCondition::BeginningOfAnyUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SacrificeUpkeepPlayerCreature],
        },
    }
}
