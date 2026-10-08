//! Candidate-owned RAV Psychic Drain identity and chosen-X spell effect.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

/// RAV #220: a blue-black sorcery with X in its mana cost. It mills the
/// targeted player for X cards and its controller gains X life.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PSYCHIC-DRAIN",
        name: "Psychic Drain",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "chosen-x-targeted-mill-and-controller-life-gain",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::MillTargetPlayerAndGainLifeControllerEqualToChosenX],
    }
}
