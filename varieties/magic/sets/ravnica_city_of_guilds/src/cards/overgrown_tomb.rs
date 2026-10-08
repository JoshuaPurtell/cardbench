//! Candidate-owned RAV Overgrown Tomb identity, mana, and entry behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, LandEntryBinding, ManaAbilityBinding,
    ManaAbilityOutput, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-OVERGROWN-TOMB",
        name: "Overgrown Tomb",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Black, Color::Green]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "optional-two-life-untapped-entry",
            "black-or-green-mana-ability",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn mana_ability() -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition: "RAV-OVERGROWN-TOMB",
        ability: ActivatedManaAbility {
            id: "produce-black-or-green",
            tap_cost: true,
            output: ManaAbilityOutput::Choice(BTreeSet::from([Color::Black, Color::Green])),
            amount: 1,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[must_use]
pub(crate) fn land_entry_binding() -> LandEntryBinding {
    LandEntryBinding {
        card_definition: "RAV-OVERGROWN-TOMB",
        enters_tapped: false,
        optional_life_payment: Some(2),
    }
}
