//! Candidate-owned RAV Chant of Vitu-Ghazi definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

/// Printed identity and executable Convoke, creature-damage prevention, and
/// life-gain behavior.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CHANT-OF-VITU-GHAZI",
        name: "Chant of Vitu-Ghazi",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(6, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "creature-damage-prevention",
            "life-gain-equal-to-damage-prevented",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![
            Effect::PreventAllCreatureDamageAndGainLifeEqualToDamagePreventedUntilEndOfTurn,
        ],
    }
}
