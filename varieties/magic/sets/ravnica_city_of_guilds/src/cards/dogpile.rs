//! Candidate-owned RAV Dogpile identity and spell effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};

/// `Dogpile` deals damage to any target equal to the number of attacking creatures.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DOGPILE",
        name: "Dogpile",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "player-or-creature-targeting",
            "attacking-creature-count-damage",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamageEqualToAttackingCreatures {
            target: TargetRequirement::PlayerOrCreature,
        }],
    }
}
