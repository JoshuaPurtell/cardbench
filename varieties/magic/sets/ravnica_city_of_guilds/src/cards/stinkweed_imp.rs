//! Candidate-owned Stinkweed Imp definition and combat-damage trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #107's black 1/2 Imp with Flying and Dredge 5.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-STINKWEED-IMP",
        name: "Stinkweed Imp",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "flying",
            "combat-damage-destroy-recipient",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![Keyword::Flying, Keyword::Dredge(5)],
        effects: vec![],
    }
}

/// Destroys the exact creature Stinkweed Imp damaged in combat.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-STINKWEED-IMP",
        ability: TriggeredAbility {
            id: "destroy-combat-damaged-creature",
            condition: TriggerCondition::DealsCombatDamageToCreature,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DestroyCombatDamagedCreature],
        },
    }
}
