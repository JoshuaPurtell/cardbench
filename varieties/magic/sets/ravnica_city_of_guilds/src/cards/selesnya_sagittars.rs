//! Candidate-owned RAV Selesnya Sagittars definition and tap ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
};

/// Printed identity and characteristics of Selesnya Sagittars.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SELESNYA-SAGITTARS",
        name: "Selesnya Sagittars",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "reach",
            "tap-damage-attacking-or-blocking-creature",
        ],
        power: Some(2),
        toughness: Some(5),
        keywords: vec![Keyword::Reach],
        effects: vec![],
    }
}

/// `{T}: Selesnya Sagittars deals 1 damage to target attacking or blocking creature.`
#[must_use]
pub(crate) fn tap_damage_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "tap-damage-attacker-or-blocker",
        mana_cost: ManaCost::new(0),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::AttackingOrBlockingCreature],
        effects: vec![Effect::DealDamage {
            amount: 1,
            target: TargetRequirement::AttackingOrBlockingCreature,
        }],
    }
}
