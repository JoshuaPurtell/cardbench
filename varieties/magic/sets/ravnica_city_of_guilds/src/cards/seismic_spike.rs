//! Candidate-owned RAV Seismic Spike definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Seismic Spike destroys a target land, then adds two red mana.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SEISMIC-SPIKE",
        name: "Seismic Spike",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red, Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "destroy-target-land",
            "add-two-red-mana",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DestroyTargetLand,
            Effect::AddManaController {
                color: Color::Red,
                amount: 2,
            },
        ],
    }
}
