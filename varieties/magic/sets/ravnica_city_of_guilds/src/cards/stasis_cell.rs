//! Candidate-owned RAV Stasis Cell definition and Aura bindings.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding, AttachmentKind, CardDefinition,
    CardType, Color, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-STASIS-CELL",
        name: "Stasis Cell",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "enchanted-creature-cannot-attack-or-block",
            "pay-three-generic-blue-to-reattach-to-target-creature",
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

/// Pay {3}{U} to attach Stasis Cell to a target creature.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-STASIS-CELL",
        ability: ActivatedAbility {
            id: "reattach-to-target-creature",
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: attachment_changes(),
            }],
        },
    }
}

/// Stasis Cell only prevents its enchanted creature from attacking or blocking.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-STASIS-CELL",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: attachment_changes(),
        granted_activated_abilities: vec![],
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)]
}
