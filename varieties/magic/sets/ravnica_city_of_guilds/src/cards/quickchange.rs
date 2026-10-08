//! Candidate-owned RAV Quickchange definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

/// Printed identity and the policy-chosen color replacement and draw effects.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-QUICKCHANGE",
        name: "Quickchange",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "policy-chosen-target-color-replacement-until-end-of-turn",
            "controller-draw",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn,
            Effect::DrawController,
        ],
    }
}
