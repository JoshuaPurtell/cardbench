//! Candidate-owned RAV Disembowel definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DISEMBOWEL",
        name: "Disembowel",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "chosen-x-targeted-creature-destruction",
            "policy-submitted-chosen-x-mana-spend",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DestroyTargetCreatureWithManaValueEqualToChosenX],
    }
}
