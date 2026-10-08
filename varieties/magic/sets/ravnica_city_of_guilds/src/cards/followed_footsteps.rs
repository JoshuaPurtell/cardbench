//! Candidate-owned RAV Followed Footsteps definition and bindings.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

/// RAV #51 is a blue `{3}{U}{U}` Aura that enchants a creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FOLLOWED-FOOTSTEPS",
        name: "Followed Footsteps",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "aura-controller-upkeep-token-copy",
            "layer-one-token-copy-provenance",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }],
    }
}

#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-FOLLOWED-FOOTSTEPS",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }
}

/// The Aura's controller gets one token copy at each of their own upkeeps
/// while the Aura remains attached to the exact enchanted creature.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FOLLOWED-FOOTSTEPS",
        ability: TriggeredAbility {
            id: "aura-controller-upkeep-token-copy",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
        },
    }
}
