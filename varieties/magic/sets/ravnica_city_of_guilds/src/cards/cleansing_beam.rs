use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Candidate-owned definition for Cleansing Beam.
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CLEANSING-BEAM",
        name: "Cleansing Beam",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "radiance",
            "targeted-creature-damage",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceDealDamageToCreatures { amount: 2 }],
    }
}
