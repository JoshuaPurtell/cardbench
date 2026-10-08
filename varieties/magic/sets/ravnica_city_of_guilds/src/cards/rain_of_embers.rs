//! CardBench-authored RAV Rain of Embers definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Printed identity and executable global damage effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-RAIN-OF-EMBERS",
        name: "Rain of Embers",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "global-creature-and-player-damage"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
    }
}
