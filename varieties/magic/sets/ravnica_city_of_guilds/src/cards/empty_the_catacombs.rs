//! Candidate-owned RAV Empty the Catacombs sorcery definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// RAV #86 returns every creature card from each player's own graveyard to
/// that player's hand, without a card-selection choice.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-EMPTY-THE-CATACOMBS",
        name: "Empty the Catacombs",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "each-player-returns-all-creature-cards-from-own-graveyard-to-hand",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ReturnAllCreatureCardsFromEachGraveyardToHand],
    }
}
