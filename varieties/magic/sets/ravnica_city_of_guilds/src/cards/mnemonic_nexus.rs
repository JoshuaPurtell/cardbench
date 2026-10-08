//! Candidate-owned RAV Mnemonic Nexus instant definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// RAV #59 costs `{3}{U}` and shuffles each player's own graveyard into that
/// player's library as the spell resolves.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MNEMONIC-NEXUS",
        name: "Mnemonic Nexus",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "graveyard-to-library",
            "per-player-library-shuffle",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ShuffleGraveyardsIntoLibraries],
    }
}
