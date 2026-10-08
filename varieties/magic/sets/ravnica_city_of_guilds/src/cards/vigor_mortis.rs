//! Candidate-owned RAV Vigor Mortis definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VIGOR-MORTIS",
        name: "Vigor Mortis",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "return-target-creature-card-from-graveyard-to-battlefield",
            "spent-green-plus-one-plus-one-counter",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                color: Color::Green,
            },
        ],
    }
}
