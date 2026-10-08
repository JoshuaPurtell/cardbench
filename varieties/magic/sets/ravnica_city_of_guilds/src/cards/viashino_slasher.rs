//! Candidate-owned RAV Viashino Slasher definition and discard-funded pump.

use cardbench_magic_engine::{ActivatedAbility, CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VIASHINO-SLASHER",
        name: "Viashino Slasher",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-plus-one-minus-one",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Pay {R} and discard a card: Viashino Slasher gets +1/-1 until end of turn.
#[must_use]
pub(crate) fn pump_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "pump-plus-one-minus-one",
        mana_cost: ManaCost::with_colors(0, [Color::Red]),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 1,
        targets: vec![],
        effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
            power: 1,
            toughness: -1,
        }],
    }
}
