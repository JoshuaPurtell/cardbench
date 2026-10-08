//! Candidate-owned Torpid Moloch definition and temporary Defender-removal ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-TORPID-MOLOCH",
        name: "Torpid Moloch",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "sacrifice-three-lands-remove-defender",
        ],
        power: Some(3),
        toughness: Some(2),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-TORPID-MOLOCH",
        ability: ActivatedAbility {
            id: "sacrifice-three-lands-remove-defender",
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 3,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::RemoveSourceKeywordUntilEndOfTurn {
                keyword: Keyword::Defender,
            }],
        },
    }
}
