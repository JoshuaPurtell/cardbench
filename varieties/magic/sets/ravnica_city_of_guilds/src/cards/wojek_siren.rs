//! Candidate-owned RAV Wojek Siren definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Wojek Siren gives the target creature and each creature sharing a color with it +1/+1.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-WOJEK-SIREN",
        name: "Wojek Siren",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "radiance", "layer-7-modifier"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceModifyPtUntilEndOfTurn {
            power: 1,
            toughness: 1,
        }],
    }
}
