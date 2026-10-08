//! Candidate-owned RAV Temple Garden identity, mana, and entry behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, LandEntryBinding, ManaAbilityBinding,
    ManaAbilityOutput, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TEMPLE-GARDEN",
        name: "Temple Garden",
        set_code: "RAV",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([Color::Green, Color::White]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "optional-two-life-untapped-entry",
            "green-or-white-mana-ability",
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
        card_definition: "RAV-TEMPLE-GARDEN",
        ability: ActivatedManaAbility {
            id: "produce-green-or-white",
            tap_cost: true,
            output: ManaAbilityOutput::Choice(BTreeSet::from([Color::Green, Color::White])),
            amount: 1,
            life_payment: None,
            controller_damage: None,
        },
    }
}

#[must_use]
pub(crate) fn land_entry_binding() -> LandEntryBinding {
    LandEntryBinding {
        card_definition: "RAV-TEMPLE-GARDEN",
        enters_tapped: false,
        optional_life_payment: Some(2),
    }
}
