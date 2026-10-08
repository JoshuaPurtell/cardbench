//! CardBench-authored RAV Boros Fury-Shield definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Printed identity and executable conditional combat-prevention effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BOROS-FURY-SHIELD",
        name: "Boros Fury-Shield",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "prevent-target-creatures-combat-damage-and-red-spend-controller-damage",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
            damage_target_controller_equal_to_power_if_mana_color_spent: Some(Color::Red),
        }],
    }
}
