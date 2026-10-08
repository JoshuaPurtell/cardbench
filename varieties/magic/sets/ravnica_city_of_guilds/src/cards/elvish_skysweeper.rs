//! Candidate-owned definition and activation for RAV Elvish Skysweeper.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

/// The printed green 1/1 creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ELVISH-SKYSWEEPER",
        name: "Elvish Skysweeper",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "sacrifice-creature-destroy-flying",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// `{4}{G}, Sacrifice a creature: destroy target creature with flying.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-ELVISH-SKYSWEEPER",
        ability: ActivatedAbility {
            id: "sacrifice-creature-destroy-flying",
            mana_cost: ManaCost::with_colors(4, [Color::Green]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 1,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::FlyingCreature],
            effects: vec![Effect::DestroyTargetFlyingCreature],
        },
    }
}
