//! Candidate-owned RAV Darkblast instant definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};
use std::collections::BTreeSet;

/// RAV #82 is a black instant with a temporary -1/-1 modifier and Dredge 3.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DARKBLAST",
        name: "Darkblast",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier", "dredge"],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Dredge(3)],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -1,
            toughness: -1,
        }],
    }
}
