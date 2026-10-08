//! Candidate-owned RAV Hour of Reckoning definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};
use std::collections::BTreeSet;

/// Printed white sorcery with Convoke that destroys all nontoken creatures.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HOUR-OF-RECKONING",
        name: "Hour of Reckoning",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White, Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "destroy-all-nontoken-creatures",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::DestroyAllNonTokenCreatures],
    }
}
