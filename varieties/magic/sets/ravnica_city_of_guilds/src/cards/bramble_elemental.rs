//! Candidate-owned RAV Bramble Elemental definition and Aura-attachment trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed identity and base characteristics for Bramble Elemental.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BRAMBLE-ELEMENTAL",
        name: "Bramble Elemental",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "optional-two-saprolings-when-aura-attaches-to-this-permanent",
        ],
        power: Some(4),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

/// Optionally creates two Saprolings whenever an Aura becomes attached to
/// this exact permanent, regardless of who controls that Aura.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-BRAMBLE-ELEMENTAL",
        ability: TriggeredAbility {
            id: "aura-attached-create-two-saprolings",
            condition: TriggerCondition::AuraBecomesAttachedToSource,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        },
    }
}
