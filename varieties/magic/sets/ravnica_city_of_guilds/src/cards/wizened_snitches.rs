//! Candidate-owned definition for RAV Wizened Snitches.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Keyword, ManaCost, StaticLibraryTopRevealBinding,
    StaticLibraryTopRevealScope,
};

/// Printed identity, Flying, and global top-library visibility behavior.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WIZENED-SNITCHES",
        name: "Wizened Snitches",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "global-static-top-library-visibility",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

/// Every player's current library top is public while this source is active.
#[must_use]
pub(crate) fn static_library_top_reveal_binding() -> StaticLibraryTopRevealBinding {
    StaticLibraryTopRevealBinding {
        card_definition: "RAV-WIZENED-SNITCHES",
        scope: StaticLibraryTopRevealScope::EveryPlayer,
    }
}
