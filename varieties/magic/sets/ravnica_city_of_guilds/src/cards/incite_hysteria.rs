//! Candidate-owned RAV Incite Hysteria definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// RAV #132: the target creature and each creature sharing one of its colors
/// cannot block for the rest of the turn.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-INCITE-HYSTERIA",
        name: "Incite Hysteria",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "radiance-cannot-block",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceAddKeywordUntilEndOfTurn {
            keyword: Keyword::CannotBlock,
        }],
    }
}
