//! Candidate-owned RAV Remand definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Counter one target spell, return it to its owner's hand, then draw a card.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-REMAND",
        name: "Remand",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-target-spell-then-draw-controller",
            "countered-spell-to-owner-hand",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::CounterTargetSpellToOwnersHand,
            Effect::DrawController,
        ],
    }
}
