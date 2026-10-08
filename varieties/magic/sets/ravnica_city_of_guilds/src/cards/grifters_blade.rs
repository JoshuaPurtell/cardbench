//! Candidate-owned RAV Grifter's Blade identity and Equipment bindings.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding, AttachmentKind, CardDefinition,
    CardType, ContinuousChange, Effect, EntryAttachmentBinding, Keyword, ManaCost,
    TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GRIFTERS-BLADE",
        name: "Grifter's Blade",
        set_code: "RAV",
        mana_cost: ManaCost::new(3),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-cost-casting",
            "flash",
            "enters-attached-choice-to-controlled-creature",
            "equipment-plus-one-plus-one",
            "sorcery-speed-equip-one",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Flash],
        effects: vec![],
    }
}

/// The controller may choose a legal creature they control during entry, or
/// decline and have Grifter's Blade enter unattached. This is not a target or
/// a triggered ability.
#[must_use]
pub(crate) fn entry_attachment_binding() -> EntryAttachmentBinding {
    EntryAttachmentBinding {
        card_definition: "RAV-GRIFTERS-BLADE",
        may_decline: true,
    }
}

/// Equip {1} at sorcery speed to a creature you control.
#[must_use]
pub(crate) fn equip_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-GRIFTERS-BLADE",
        ability: ActivatedAbility {
            id: "equip-plus-one-plus-one",
            mana_cost: ManaCost::new(1),
            tap_cost: false,
            sorcery_speed: true,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::ControlledCreature],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::ControlledCreature,
                changes: vec![ContinuousChange::ModifyPowerToughness {
                    power: 1,
                    toughness: 1,
                }],
            }],
        },
    }
}

/// The attached creature receives the Equipment's continuous +1/+1 bonus.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-GRIFTERS-BLADE",
        kind: AttachmentKind::Equipment,
        target: TargetRequirement::ControlledCreature,
        changes: vec![ContinuousChange::ModifyPowerToughness {
            power: 1,
            toughness: 1,
        }],
        granted_activated_abilities: vec![],
    }
}
