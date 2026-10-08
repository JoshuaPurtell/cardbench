//! Candidate-owned definition and activated ability for Mortipede.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MORTIPEDE",
        name: "Mortipede",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-green-must-be-blocked",
        ],
        power: Some(4),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-MORTIPEDE",
        ability: ActivatedAbility {
            id: "green-must-be-blocked",
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::AddSourceKeywordUntilEndOfTurn {
                keyword: Keyword::MustBeBlockedIfAble,
            }],
        },
    }
}
