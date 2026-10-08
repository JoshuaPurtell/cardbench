//! Candidate-owned RAV Galvanic Arc definition and triggered/attachment rules.

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Color, ContinuousChange, Effect,
    Keyword, ManaCost, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed identity, Aura attachment instruction, and cast cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GALVANIC-ARC",
        name: "Galvanic Arc",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "etb-three-damage-to-target-creature-or-player",
            "attached-creature-first-strike",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: attachment_changes(),
        }],
    }
}

/// The mandatory ETB trigger deals three damage to its creature-or-player target.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GALVANIC-ARC",
        ability: TriggeredAbility {
            id: "etb-deal-three",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::PlayerOrCreature],
            effects: vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::PlayerOrCreature,
            }],
        },
    }
}

/// While attached, Galvanic Arc grants First Strike to the enchanted creature.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-GALVANIC-ARC",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: attachment_changes(),
        granted_activated_abilities: vec![],
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::AddKeyword(Keyword::FirstStrike)]
}
