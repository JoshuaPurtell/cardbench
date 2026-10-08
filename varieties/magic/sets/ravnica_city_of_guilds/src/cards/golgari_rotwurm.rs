//! Candidate-owned RAV Golgari Rotwurm definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-ROTWURM",
        name: "Golgari Rotwurm",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-sacrifice-creature-target-player-life-loss",
        ],
        power: Some(5),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-GOLGARI-ROTWURM",
        ability: ActivatedAbility {
            id: "sacrifice-creature-target-player-life-loss",
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 1,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Player],
            effects: vec![Effect::LoseLifeTarget { amount: 1 }],
        },
    }
}
