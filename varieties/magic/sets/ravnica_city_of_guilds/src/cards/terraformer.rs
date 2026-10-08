//! Candidate-owned definition and activated ability for the RAV Terraformer.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TERRAFORMER",
        name: "Terraformer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controller-lands-chosen-basic-land-type-until-end-of-turn",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TERRAFORMER",
        ability: ActivatedAbility {
            id: "choose-controller-land-basic-type",
            mana_cost: ManaCost::with_colors(1, []),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::ReplaceControllerLandsWithChosenBasicLandTypeUntilEndOfTurn],
        },
    }
}
