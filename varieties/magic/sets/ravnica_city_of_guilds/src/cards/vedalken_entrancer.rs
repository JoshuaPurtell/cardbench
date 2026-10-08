//! Candidate-owned RAV Vedalken Entrancer identity and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VEDALKEN-ENTRANCER",
        name: "Vedalken Entrancer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "defender",
            "tap-blue-target-player-mill-two",
        ],
        power: Some(1),
        toughness: Some(4),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "tap-blue-mill-two",
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        tap_cost: true,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Player],
        effects: vec![Effect::MillTargetPlayer { count: 2 }],
    }
}
