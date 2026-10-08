//! Candidate-owned definition for RAV Muddle the Mixture.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed instant, counterspell effect, and hand-zone Transmute cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-MUDDLE-THE-MIXTURE",
        name: "Muddle the Mixture",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-target-instant-or-sorcery-spell",
            "transmute",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
    }
}
