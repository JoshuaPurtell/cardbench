//! Candidate-owned RAV Peregrine Mask definition and equipment bindings.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding, AttachmentKind, CardDefinition,
    CardType, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PEREGRINE-MASK",
        name: "Peregrine Mask",
        set_code: "RAV",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-equipment-casting",
            "equipment-defender-flying-first-strike",
            "sorcery-speed-equip-two",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Peregrine Mask can equip one of its controller's creatures for {2} at sorcery speed.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-PEREGRINE-MASK",
        ability: ActivatedAbility {
            id: "equip-defender-flying-first-strike",
            mana_cost: ManaCost::new(2),
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
                changes: attachment_changes(),
            }],
        },
    }
}

/// While attached, the equipped creature has Defender, Flying, and First Strike.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-PEREGRINE-MASK",
        kind: AttachmentKind::Equipment,
        target: TargetRequirement::ControlledCreature,
        changes: attachment_changes(),
        granted_activated_abilities: vec![],
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![
        ContinuousChange::AddKeyword(Keyword::Defender),
        ContinuousChange::AddKeyword(Keyword::Flying),
        ContinuousChange::AddKeyword(Keyword::FirstStrike),
    ]
}
