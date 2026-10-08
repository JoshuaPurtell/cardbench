//! Candidate-owned RAV Smash definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Smash destroys a target artifact and then lets its caster draw one card.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SMASH",
        name: "Smash",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "artifact-destruction", "draw"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DestroyTargetArtifact, Effect::DrawController],
    }
}
