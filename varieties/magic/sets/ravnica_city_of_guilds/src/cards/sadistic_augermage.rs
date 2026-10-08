//! Candidate-owned RAV #103 Sadistic Augermage identity and death trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SADISTIC-AUGERMAGE",
        name: "Sadistic Augermage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "source-dies-each-player-puts-card-on-library-top",
            "policy-private-each-player-hand-selection",
        ],
        power: Some(3),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SADISTIC-AUGERMAGE",
        ability: TriggeredAbility {
            id: "source-dies-each-player-puts-card-on-library-top",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::PutOneHandCardOnLibraryTopEachPlayer],
        },
    }
}
