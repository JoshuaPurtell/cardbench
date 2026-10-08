//! Candidate-owned definition and linked-hand upkeep abilities for Bottled Cloister.

use cardbench_magic_engine::{
    CardDefinition, CardType, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BOTTLED-CLOISTER",
        name: "Bottled Cloister",
        set_code: "RAV",
        mana_cost: ManaCost::new(4),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "opponent-upkeep-linked-hand-exile",
            "controller-upkeep-linked-hand-return-then-draw",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn opponent_upkeep_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-BOTTLED-CLOISTER",
        ability: TriggeredAbility {
            id: "opponent-upkeep-exile-controller-hand-linked",
            condition: TriggerCondition::BeginningOfOpponentsUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::ExileControllerHandLinkedToSource],
        },
    }
}

#[must_use]
pub(crate) fn controller_upkeep_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-BOTTLED-CLOISTER",
        ability: TriggeredAbility {
            id: "controller-upkeep-return-linked-hand-then-draw",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::ReturnLinkedHandExileToControllerHand,
                Effect::DrawController,
            ],
        },
    }
}
