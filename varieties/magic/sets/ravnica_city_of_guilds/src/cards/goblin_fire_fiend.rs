//! Candidate-owned definition and activated ability for Goblin Fire Fiend.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Color, Effect, Keyword, ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOBLIN-FIRE-FIEND",
        name: "Goblin Fire Fiend",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "haste",
            "must-block-if-able",
            "activated-plus-one-power",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Haste, Keyword::MustBeBlockedIfAble],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn pump_plus_one_power_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "pump-plus-one-power",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
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
            toughness: 0,
        }],
    }
}
