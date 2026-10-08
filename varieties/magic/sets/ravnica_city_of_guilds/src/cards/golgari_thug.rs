//! Candidate-owned Golgari Thug definition and graveyard death trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #87's black 1/1 with Dredge 4.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-THUG",
        name: "Golgari Thug",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "dies-target-creature-card-owner-library-top",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Dredge(4)],
        effects: vec![],
    }
}

/// When this creature dies, its controller targets a creature card in their
/// graveyard and puts that card on top of its owner's library.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GOLGARI-THUG",
        ability: TriggeredAbility {
            id: "dies-target-creature-card-owner-library-top",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::CreatureCardInControllerGraveyard],
            effects: vec![Effect::PutTargetCreatureCardInControllerGraveyardOnOwnersLibraryTop],
        },
    }
}
