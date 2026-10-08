//! Candidate-owned RAV Dark Heart of the Wood definition and Forest-cost ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostBinding, BasicLandType,
    CardDefinition, CardType, Color, Effect, GeneralizedActivatedAbilityCost, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DARK-HEART-OF-THE-WOOD",
        name: "Dark Heart of the Wood",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "activated-green-sacrifice-forest-gain-three-life",
            "typed-basic-land-sacrifice-cost",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-DARK-HEART-OF-THE-WOOD",
        ability: ActivatedAbility {
            id: "green-sacrifice-forest-gain-three-life",
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 1,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 3 }],
        },
    }
}

#[must_use]
pub(crate) fn activated_ability_cost() -> ActivatedAbilityCostBinding {
    ActivatedAbilityCostBinding {
        card_definition: "RAV-DARK-HEART-OF-THE-WOOD",
        ability_id: "green-sacrifice-forest-gain-three-life",
        cost: GeneralizedActivatedAbilityCost {
            sacrifice_land_basic_type: Some(BasicLandType::Forest),
            ..GeneralizedActivatedAbilityCost::default()
        },
    }
}
