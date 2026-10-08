//! Candidate-owned RAV Leashling definition and hand-cost return ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostBinding, CardDefinition,
    CardType, Effect, GeneralizedActivatedAbilityCost, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LEASHLING",
        name: "Leashling",
        set_code: "RAV",
        mana_cost: ManaCost::new(6),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact, CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-creature-casting",
            "base-characteristics",
            "hand-card-top-library-cost-return-source",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-LEASHLING",
        ability: ActivatedAbility {
            id: "hand-card-library-top-return-source",
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::ReturnSourceToOwnersHand],
        },
    }
}

#[must_use]
pub(crate) fn activated_ability_cost() -> ActivatedAbilityCostBinding {
    ActivatedAbilityCostBinding {
        card_definition: "RAV-LEASHLING",
        ability_id: "hand-card-library-top-return-source",
        cost: GeneralizedActivatedAbilityCost {
            put_hand_cards_on_library_top: 1,
            ..GeneralizedActivatedAbilityCost::default()
        },
    }
}
