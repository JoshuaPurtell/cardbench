//! Candidate-owned RAV Shadow of Doubt definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, HybridManaSymbol, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SHADOW-OF-DOUBT",
        name: "Shadow of Doubt",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            0,
            [],
            [
                HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                },
                HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                },
            ],
        ),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "library-search-prevention", "draw"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::PreventLibrarySearchUntilEndOfTurn,
            Effect::DrawController,
        ],
    }
}
