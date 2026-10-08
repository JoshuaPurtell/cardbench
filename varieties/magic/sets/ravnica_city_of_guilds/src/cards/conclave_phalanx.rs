//! Candidate-owned definition and ETB life-gain trigger for Conclave Phalanx.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CONCLAVE-PHALANX",
        name: "Conclave Phalanx",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "etb-life-per-controlled-creature",
        ],
        power: Some(2),
        toughness: Some(4),
        keywords: vec![Keyword::Convoke],
        effects: vec![],
    }
}

/// Gain one life for each creature controlled when this trigger resolves.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-CONCLAVE-PHALANX",
        ability: TriggeredAbility {
            id: "etb-gain-life-per-controlled-creature",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeForEachControlledCreature],
        },
    }
}
