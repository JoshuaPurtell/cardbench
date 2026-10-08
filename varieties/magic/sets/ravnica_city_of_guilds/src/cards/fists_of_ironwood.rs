//! Candidate-owned RAV Fists of Ironwood definition and ETB trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, Effect, ManaCost, TargetRequirement,
    TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed identity and executable Creature Aura attachment behavior.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FISTS-OF-IRONWOOD",
        name: "Fists of Ironwood",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "aura-enchant-creature-trample-etb-two-saprolings",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(
                cardbench_magic_engine::Keyword::Trample,
            )],
        }],
    }
}

/// Creates two Saprolings whenever this Aura enters the battlefield.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FISTS-OF-IRONWOOD",
        ability: TriggeredAbility {
            id: "etb-two-saprolings",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        },
    }
}
