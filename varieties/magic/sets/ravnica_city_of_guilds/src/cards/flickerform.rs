//! Candidate-owned RAV Flickerform definition and linked-exile activation.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};
use std::collections::BTreeSet;

/// Printed identity and Aura attachment restriction.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FLICKERFORM",
        name: "Flickerform",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "aura-linked-exile-and-next-end-step-return",
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

/// Exiles the enchanted creature and attached Auras until the next end step.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-FLICKERFORM",
        ability: ActivatedAbility {
            id: "linked-exile-attached-creature-and-auras",
            mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
        },
    }
}
