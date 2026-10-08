//! Candidate-owned RAV Auratouched Mage definition and Aura search binding.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, LibrarySearchSelection, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-AURATOUCHED-MAGE",
        name: "Auratouched Mage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-aura-search-and-attach",
            "policy-selected-compatible-aura-selection",
            "optional-compatible-aura-search",
        ],
        power: Some(3),
        toughness: Some(3),
        keywords: vec![],
        effects: vec![],
    }
}

/// Its mandatory ETB trigger lets its controller choose a compatible Aura
/// privately, or fail to find one, and attaches a selected Aura to this Mage.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-AURATOUCHED-MAGE",
        ability: TriggeredAbility {
            id: "etb-search-and-attach-aura",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::SearchControllerLibraryForCompatibleAuraAttachedToSource {
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                },
            ],
        },
    }
}
