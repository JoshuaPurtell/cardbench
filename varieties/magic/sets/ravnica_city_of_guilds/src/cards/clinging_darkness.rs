//! Candidate-owned RAV Clinging Darkness definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CLINGING-DARKNESS",
        name: "Clinging Darkness",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "aura-attach-and-static-pt"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceAndModifyTargetPt {
            power: -3,
            toughness: -1,
        }],
    }
}
