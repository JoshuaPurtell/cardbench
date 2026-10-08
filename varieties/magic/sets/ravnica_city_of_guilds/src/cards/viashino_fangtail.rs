//! Candidate-owned RAV Viashino Fangtail definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};

/// Printed identity and characteristics of Viashino Fangtail.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VIASHINO-FANGTAIL",
        name: "Viashino Fangtail",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-deal-one-to-player-or-creature",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// Stack-backed tap ability dealing one damage to a legal player or creature.
#[must_use]
pub(crate) fn tap_deal_one_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "tap-deal-one-to-player-or-creature",
        mana_cost: ManaCost::new(0),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::PlayerOrCreature],
        effects: vec![Effect::DealDamage {
            amount: 1,
            target: TargetRequirement::PlayerOrCreature,
        }],
    }
}
