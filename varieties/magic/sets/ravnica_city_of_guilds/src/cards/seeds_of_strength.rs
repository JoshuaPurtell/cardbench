//! Candidate-owned RAV Seeds of Strength definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

/// Printed identity and the spell's three independently targeted +1/+1 effects.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SEEDS-OF-STRENGTH",
        name: "Seeds of Strength",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "three-targeted-layer-7-modifiers",
            "partial-target-resolution",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
            Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            },
        ],
    }
}
