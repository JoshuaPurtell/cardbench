//! Candidate-owned RAV Vitu-Ghazi identity and activated bindings.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedManaAbility, CardDefinition, CardType, Color, Effect,
    ManaAbilityOutput, ManaCost, TokenSpec,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VITU-GHAZI",
        name: "Vitu-Ghazi, the City-Tree",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Colorless]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-mana-ability",
            "activated-green-saproling-token",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn colorless_mana_ability() -> ActivatedManaAbility {
    ActivatedManaAbility {
        id: "produce-colorless",
        tap_cost: true,
        output: ManaAbilityOutput::Fixed(Color::Colorless),
        amount: 1,
        life_payment: None,
        controller_damage: None,
    }
}

#[must_use]
pub(crate) fn create_saproling_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "create-green-saproling",
        mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
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
