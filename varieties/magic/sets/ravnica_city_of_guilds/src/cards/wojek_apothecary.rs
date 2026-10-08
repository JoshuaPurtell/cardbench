//! Candidate-owned RAV Wojek Apothecary definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement,
};

/// Printed identity for RAV Wojek Apothecary (#36).
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WOJEK-APOTHECARY",
        name: "Wojek Apothecary",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-radiance-prevent-one-damage",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// `{T}: Prevent the next 1 damage that would be dealt to target creature
/// this turn and each other creature that shares a color with it.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-WOJEK-APOTHECARY",
        ability: ActivatedAbility {
            id: "tap-radiance-prevent-one-damage",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::RadianceAddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
        },
    }
}
