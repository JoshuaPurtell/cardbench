//! Candidate-owned RAV Suppression Field definition and live ability tax.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding, CardDefinition, CardType,
    Color, ManaCost,
};

/// Printed white enchantment identity.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SUPPRESSION-FIELD",
        name: "Suppression Field",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "static-nonmana-activated-ability-tax",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// Taxes each nonmana activated ability by {2} while this enchantment is live.
#[must_use]
pub(crate) fn cost_modifier_binding() -> ActivatedAbilityCostModifierBinding {
    ActivatedAbilityCostModifierBinding {
        source_definition: "RAV-SUPPRESSION-FIELD",
        modifier: ActivatedAbilityCostModifier::IncreaseGeneric {
            amount: 2,
            nonmana_only: true,
        },
    }
}
