//! CardBench-authored Char definition, isolated for deterministic task removal.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TargetRequirement,
};

/// Complete currently represented Char functionality.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CHAR",
        name: "Char",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-damage", "self-damage"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DealDamage {
                amount: 4,
                target: TargetRequirement::PlayerOrCreature,
            },
            Effect::DealDamageController { amount: 2 },
        ],
    }
}
