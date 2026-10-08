//! Candidate-owned RAV Flight of Fancy definition and ETB draw binding.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Effect, Keyword, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FLIGHT-OF-FANCY",
        name: "Flight of Fancy",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-enchant-creature-flying-etb-draw-two",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::Flying)],
        }],
    }
}

/// Flight of Fancy draws two cards when it enters the battlefield.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FLIGHT-OF-FANCY",
        ability: TriggeredAbility {
            id: "etb-draw-two",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DrawController, Effect::DrawController],
        },
    }
}
