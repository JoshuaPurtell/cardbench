//! Candidate-owned RAV Rally the Righteous definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Printed identity, casting cost, and Radiance effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-RALLY-THE-RIGHTEOUS",
        name: "Rally the Righteous",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "radiance",
            "untap",
            "layer-7-modifier",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
            power: 2,
            toughness: 0,
        }],
    }
}
