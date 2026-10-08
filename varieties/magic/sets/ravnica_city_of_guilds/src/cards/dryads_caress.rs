//! Candidate-owned RAV Dryad's Caress identity and spell effects.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Counts battlefield creatures for life and conditionally untaps the controller's creatures.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DRYADS-CARESS",
        name: "Dryad's Caress",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "battlefield-creature-life-gain-and-white-spend-untap",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::GainLifeForEachCreature,
            Effect::UntapControllerCreaturesIfManaColorSpent {
                color: Color::White,
            },
        ],
    }
}
