//! CardBench-authored RAV Roofstalker Wight identity and self-Flying binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ROOFSTALKER-WIGHT",
        name: "Roofstalker Wight",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "self-flying-until-end-of-turn",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

/// Candidate-owned `{1}{U}` activation granting Flying to this creature for the turn.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "blue-gain-flying",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::AddSourceKeywordUntilEndOfTurn {
            keyword: Keyword::Flying,
        }],
    }
}
