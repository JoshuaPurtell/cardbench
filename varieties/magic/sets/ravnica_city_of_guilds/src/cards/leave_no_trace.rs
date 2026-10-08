use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};

/// Candidate-owned definition for Leave No Trace.
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-LEAVE-NO-TRACE",
        name: "Leave No Trace",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "radiance-enchantment-destruction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RadianceDestroyEnchantments],
    }
}
