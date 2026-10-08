//! Candidate-owned RAV Undercity Shade identity and black-mana pump binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-UNDERCITY-SHADE",
        name: "Undercity Shade",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "black-only-evasion",
            "black-mana-self-pump",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::BlackEvasion],
        effects: vec![],
    }
}

/// {B}: This creature gets +1/+1 until end of turn.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "pump-plus-one-plus-one",
        mana_cost: ManaCost::with_colors(0, [Color::Black]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 1,
            toughness: 1,
        }],
    }
}
