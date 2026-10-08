//! Candidate-owned RAV Sundering Vitae definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SUNDERING-VITAE",
        name: "Sundering Vitae",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "targeted-artifact-or-enchantment-destruction",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::DestroyTargetArtifactOrEnchantment],
    }
}
