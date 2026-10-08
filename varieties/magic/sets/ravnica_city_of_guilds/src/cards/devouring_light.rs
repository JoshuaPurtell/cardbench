//! Candidate-owned RAV Devouring Light definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed identity and spell effect for Devouring Light.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DEVOURING-LIGHT",
        name: "Devouring Light",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "attacking-or-blocking-creature-target",
            "exile",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::ExileTargetPermanent],
    }
}
