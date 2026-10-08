//! Candidate-owned RAV Elves of Deep Shadow definition and mana ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, ManaAbilityBinding, ManaAbilityOutput,
    ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ELVES-OF-DEEP-SHADOW",
        name: "Elves of Deep Shadow",
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
            "bound-tap-black-mana-ability",
            "source-aware-controller-damage",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn mana_ability() -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition: "RAV-ELVES-OF-DEEP-SHADOW",
        ability: ActivatedManaAbility {
            id: "produce-black-and-damage-controller",
            tap_cost: true,
            output: ManaAbilityOutput::Fixed(Color::Black),
            amount: 1,
            life_payment: None,
            controller_damage: Some(1),
        },
    }
}
