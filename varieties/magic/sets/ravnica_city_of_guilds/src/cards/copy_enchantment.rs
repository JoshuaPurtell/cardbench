//! Candidate-owned RAV Copy Enchantment card definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, ManaCost};

/// RAV #42 is a blue `{2}{U}` enchantment. Its entry-copy choice is wired
/// through the set's typed `EntryCopyBinding` registry.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-COPY-ENCHANTMENT",
        name: "Copy Enchantment",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "may-enter-as-copy-of-enchantment",
            "public-entry-copy-decision",
            "copied-aura-entry-attachment",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}
