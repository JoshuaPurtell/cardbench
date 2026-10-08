//! Candidate-owned RAV Putrefy instant definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PUTREFY",
        name: "Putrefy",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "artifact-or-creature-destruction-no-regeneration",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DestroyTargetArtifactOrCreatureNoRegeneration],
    }
}
