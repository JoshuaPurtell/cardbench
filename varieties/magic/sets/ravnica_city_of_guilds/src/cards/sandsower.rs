//! Candidate-owned definition and three-creature activation for RAV Sandsower.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SANDSOWER",
        name: "Sandsower",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-three-untapped-controlled-creatures",
            "tap-target-creature",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-SANDSOWER",
        ability: ActivatedAbility {
            id: "tap-target-creature",
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 3,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::TapTargetCreature],
        },
    }
}
