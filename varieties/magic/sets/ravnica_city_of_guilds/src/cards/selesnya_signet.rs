//! Candidate-owned RAV Selesnya Signet definition and paid mana binding.

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, Color, ManaAbilityBinding, ManaAbilityOutput,
    ManaBundle, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SELESNYA-SIGNET",
        name: "Selesnya Signet",
        set_code: "RAV",
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "artifact-casting",
            "paid-fixed-two-color-mana-ability",
            "cast-payment-mana-activation",
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
        card_definition: "RAV-SELESNYA-SIGNET",
        ability: ActivatedManaAbility {
            id: "selesnya-signet-gw",
            tap_cost: true,
            output: ManaAbilityOutput::PaidBundle {
                mana_cost: ManaCost::new(1),
                bundle: ManaBundle::new([(Color::Green, 1), (Color::White, 1)]),
            },
            amount: 0,
            life_payment: None,
            controller_damage: None,
        },
    }
}
