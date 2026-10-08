//! Candidate-owned RAV Carven Caryatid definition and ETB draw binding.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CARVEN-CARYATID",
        name: "Carven Caryatid",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "enter-the-battlefield-draw",
        ],
        power: Some(2),
        toughness: Some(5),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

/// Carven Caryatid's mandatory enter-the-battlefield draw trigger.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-CARVEN-CARYATID",
        ability: TriggeredAbility {
            id: "etb-draw-controller",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}
