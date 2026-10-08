//! Candidate-owned RAV Greater Forgeling definition and self-pump ability.

use cardbench_magic_engine::{ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GREATER-FORGELING",
        name: "Greater Forgeling",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-plus-three-minus-three",
        ],
        power: Some(3),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

/// Pay {1}{R}: Greater Forgeling gets +3/-3 until end of turn.
#[must_use]
pub(crate) fn pump_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "pump-plus-three-minus-three",
        mana_cost: ManaCost::with_colors(1, [Color::Red]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![],
        effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 3,
            toughness: -3,
        }],
    }
}
