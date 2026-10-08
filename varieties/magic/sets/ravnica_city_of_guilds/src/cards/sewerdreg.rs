//! Candidate-owned definition and self-regeneration ability for Sewerdreg.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SEWERDREG",
        name: "Sewerdreg",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "fear",
            "regeneration",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![Keyword::Fear],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-SEWERDREG",
        ability: ActivatedAbility {
            id: "self-regeneration",
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::RegenerateSource],
        },
    }
}
