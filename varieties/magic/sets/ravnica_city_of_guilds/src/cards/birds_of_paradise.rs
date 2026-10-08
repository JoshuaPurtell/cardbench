//! Candidate-owned RAV Birds of Paradise definition and chosen-color mana ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, Keyword, ManaAbilityBinding,
    ManaAbilityOutput, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BIRDS-OF-PARADISE",
        name: "Birds of Paradise",
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
            "flying",
            "tap-choice-single-color-mana-ability",
            "cast-payment-mana-activation",
        ],
        power: Some(0),
        toughness: Some(1),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn mana_ability() -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition: "RAV-BIRDS-OF-PARADISE",
        ability: ActivatedManaAbility {
            id: "produce-one-color",
            tap_cost: true,
            output: ManaAbilityOutput::Choice(Color::ALL.into_iter().collect()),
            amount: 1,
            life_payment: None,
            controller_damage: None,
        },
    }
}
