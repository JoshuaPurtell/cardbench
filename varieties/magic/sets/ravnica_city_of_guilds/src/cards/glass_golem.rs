//! Candidate implementation for the RAV Glass Golem pilot.

use cardbench_magic_engine::{CardDefinition, CardType, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GLASS-GOLEM",
        name: "Glass Golem",
        set_code: "RAV",
        mana_cost: ManaCost::new(5),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact, CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-cost-casting",
            "artifact-creature-base-characteristics",
        ],
        power: Some(6),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}
