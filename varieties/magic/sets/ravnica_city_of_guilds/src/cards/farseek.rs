//! Candidate-owned RAV Farseek definition.

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, Color, Effect, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FARSEEK",
        name: "Farseek",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "nonforest-basic-land-type-search",
            "private-library-selection",
            "battlefield-tapped-land-entry",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::SearchControllerLibrary {
            requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                BasicLandType::Plains,
                BasicLandType::Island,
                BasicLandType::Swamp,
                BasicLandType::Mountain,
            ])),
            destination: LibrarySearchDestination::BattlefieldTapped,
            selection: LibrarySearchSelection::PolicySubmitted {
                may_fail_to_find: true,
            },
            reveal_selected: false,
        }],
    }
}
