//! CardBench-authored RAV Selesnya Evangel identity and activated-ability binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost, TokenSpec,
};

/// Printed characteristics for the RAV green-white creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SELESNYA-EVANGEL",
        name: "Selesnya Evangel",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-token-creation-with-creature-tap-cost",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// The task-local binding lets the authority exercise the submitted ability.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "create-saproling",
        mana_cost: ManaCost::new(1),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 1,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 1,
        }],
    }
}
