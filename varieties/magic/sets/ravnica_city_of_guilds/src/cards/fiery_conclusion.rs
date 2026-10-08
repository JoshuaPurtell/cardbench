//! CardBench-authored RAV Fiery Conclusion definition and additional-cost binding.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AdditionalSpellCost, AdditionalSpellCostBinding, CardDefinition, CardType, Color, Effect,
    ManaCost, TargetRequirement,
};

/// Printed identity and executable targeted damage effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FIERY-CONCLUSION",
        name: "Fiery Conclusion",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "additional-sacrifice-controlled-creature-cost",
            "targeted-creature-damage",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamage {
            amount: 5,
            target: TargetRequirement::Creature,
        }],
    }
}

/// Binds the required controlled-creature sacrifice as an additional cast cost.
#[must_use]
pub(crate) fn additional_spell_cost_binding() -> AdditionalSpellCostBinding {
    AdditionalSpellCostBinding {
        card_definition: "RAV-FIERY-CONCLUSION",
        cost: AdditionalSpellCost::SacrificeControlledCreature,
    }
}
