//! Candidate-owned RAV Bathe in Light definition.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-BATHE-IN-LIGHT",
        name: "Bathe in Light",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "targeted-radiance-chosen-color-protection",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::TargetedBundle {
            target: TargetRequirement::Creature,
            effects: vec![
                Effect::RadianceAddChosenColorProtectionToTargetAndSharedColorCreaturesUntilEndOfTurn,
            ],
        }],
    }
}
