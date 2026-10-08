//! CardBench-authored Lightning Helix definition, isolated for trace qualification.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};

/// Complete currently represented Lightning Helix functionality.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LIGHTNING-HELIX",
        name: "Lightning Helix",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::White, Color::Red]),
        colors: BTreeSet::from([Color::White, Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-damage", "life-gain"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::PlayerOrCreature,
            },
            Effect::GainLifeController { amount: 3 },
        ],
    }
}
