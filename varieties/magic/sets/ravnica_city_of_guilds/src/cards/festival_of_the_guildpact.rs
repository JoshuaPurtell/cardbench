//! Candidate-owned RAV Festival of the Guildpact definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FESTIVAL-OF-THE-GUILDPACT",
        name: "Festival of the Guildpact",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "chosen-x-controller-damage-prevention-and-draw",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn,
            Effect::DrawController,
        ],
    }
}
