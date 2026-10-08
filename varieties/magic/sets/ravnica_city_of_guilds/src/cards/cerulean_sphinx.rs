//! Candidate-owned RAV Cerulean Sphinx definition and owner-relative shuffle activation.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CERULEAN-SPHINX",
        name: "Cerulean Sphinx",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "activated-source-owner-library-shuffle",
        ],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-CERULEAN-SPHINX",
        ability: ActivatedAbility {
            id: "shuffle-source-into-owner-library",
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::MoveSourceToOwnersLibraryAndShuffle],
        },
    }
}
