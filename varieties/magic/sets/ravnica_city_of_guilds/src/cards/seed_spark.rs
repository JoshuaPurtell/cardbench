//! Candidate-owned RAV Seed Spark definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost, TokenSpec};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SEED-SPARK",
        name: "Seed Spark",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "typed-artifact-or-enchantment-target",
            "destroy",
            "saproling-token-creation",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::DestroyTargetArtifactOrEnchantment,
            Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            },
        ],
    }
}
