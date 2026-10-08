//! Candidate-owned RAV Agrus Kos, Wojek Veteran definition and attack trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

/// RAV #190: a legendary red-white 3/3 Human Soldier for `{3}{R}{W}`.
/// Legendary-supertype metadata is supplied by the set binding registry.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-AGRUS-KOS-WOJEK-VETERAN",
        name: "Agrus Kos, Wojek Veteran",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "attack-triggered-color-specific-combat-modifiers",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// When Agrus attacks, each attacking red creature gets +2/+0 and each
/// attacking white creature gets +0/+2 until end of turn. A red-white
/// attacker, including Agrus, receives both independent effects.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-AGRUS-KOS-WOJEK-VETERAN",
        ability: TriggeredAbility {
            id: "attack-color-specific-combat-modifiers",
            condition: TriggerCondition::Attacks,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                    color: Color::Red,
                    power: 2,
                    toughness: 0,
                },
                Effect::ModifyAttackingCreaturesOfColorUntilEndOfTurn {
                    color: Color::White,
                    power: 0,
                    toughness: 2,
                },
            ],
        },
    }
}
