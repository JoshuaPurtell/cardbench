//! Candidate-owned RAV Lore Broker definition and activated ability.

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, ManaCost,
};
use std::collections::BTreeSet;

/// Printed blue 1/2 creature with an each-player draw-and-discard tap ability.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LORE-BROKER",
        name: "Lore Broker",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-activated-each-player-draws-one",
            "simultaneous-private-each-player-discard-after-draw",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Each player draws one card, then each chooses and discards one privately.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-LORE-BROKER",
        ability: ActivatedAbility {
            id: "each-player-draws-then-discards",
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::EachPlayerDrawsThenDiscardsOneCard],
        },
    }
}
