//! Candidate-owned RAV Carrion Howler definition and life-paid pump.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityCostBinding, CardDefinition, CardType, Color, Effect,
    GeneralizedActivatedAbilityCost, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CARRION-HOWLER",
        name: "Carrion Howler",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "life-paid-source-plus-two-minus-one",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Pay 1 life: Carrion Howler gets +2/-1 until end of turn.
#[must_use]
pub(crate) fn life_pump_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "pay-life-pump-plus-two-minus-one",
        mana_cost: ManaCost::new(0),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 2,
            toughness: -1,
        }],
    }
}

#[must_use]
pub(crate) fn life_pump_cost() -> ActivatedAbilityCostBinding {
    ActivatedAbilityCostBinding {
        card_definition: "RAV-CARRION-HOWLER",
        ability_id: "pay-life-pump-plus-two-minus-one",
        cost: GeneralizedActivatedAbilityCost {
            life_payment: 1,
            ..GeneralizedActivatedAbilityCost::default()
        },
    }
}
