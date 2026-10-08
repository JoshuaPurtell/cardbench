//! Candidate-owned RAV Surveilling Sprite definition and optional dies trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SURVEILLING-SPRITE",
        name: "Surveilling Sprite",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "dies-draw-controller",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// The controller may draw a card when Surveilling Sprite dies.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SURVEILLING-SPRITE",
        ability: TriggeredAbility {
            id: "dies-draw-controller",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}
