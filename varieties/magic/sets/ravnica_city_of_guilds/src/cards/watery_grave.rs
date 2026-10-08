//! Candidate-owned RAV Watery Grave identity, mana, and entry behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, LandEntryBinding, ManaAbilityBinding,
    ManaAbilityOutput, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WATERY-GRAVE",
        name: "Watery Grave",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Blue, Color::Black]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "optional-two-life-untapped-entry",
            "blue-or-black-mana-ability",
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
        card_definition: "RAV-WATERY-GRAVE",
        ability: ActivatedManaAbility {
            id: "produce-blue-or-black",
            tap_cost: true,
            output: ManaAbilityOutput::Choice(BTreeSet::from([Color::Blue, Color::Black])),
            amount: 1,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[must_use]
pub(crate) fn land_entry_binding() -> LandEntryBinding {
    LandEntryBinding {
        card_definition: "RAV-WATERY-GRAVE",
        enters_tapped: false,
        optional_life_payment: Some(2),
    }
}
