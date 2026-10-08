//! Candidate-owned RAV #177 Primordial Sage definition and cast trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PRIMORDIAL-SAGE",
        name: "Primordial Sage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controller-casts-creature-spell-may-draw",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-PRIMORDIAL-SAGE",
        ability: TriggeredAbility {
            id: "controller-creature-spell-cast-may-draw",
            condition: TriggerCondition::CastsCreatureSpell,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}
