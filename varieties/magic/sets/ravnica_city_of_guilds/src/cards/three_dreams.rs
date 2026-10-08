//! Candidate-owned RAV Three Dreams definition and private Aura search.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, LibrarySearchCardinality, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-THREE-DREAMS",
        name: "Three Dreams",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "private-up-to-three-distinct-name-aura-search-reveal-hand-shuffle",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::SearchControllerLibraryMany {
            requirement: LibrarySearchRequirement::Aura,
            destination: LibrarySearchDestination::Hand,
            cardinality: LibrarySearchCardinality::ZeroOrMoreDistinctNames { maximum: 3 },
            selection: LibrarySearchSelection::PolicySubmitted {
                may_fail_to_find: false,
            },
            reveal_selected: true,
        }],
    }
}
