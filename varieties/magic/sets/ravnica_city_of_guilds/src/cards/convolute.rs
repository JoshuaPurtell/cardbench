//! Candidate-owned RAV Convolute definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CONVOLUTE",
        name: "Convolute",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "counter-target-spell-unless-controller-pays-4",
            "policy-submitted-resolution-payment",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::CounterTargetSpellUnlessControllerPays {
            mana_cost: ManaCost::new(4),
        }],
    }
}
