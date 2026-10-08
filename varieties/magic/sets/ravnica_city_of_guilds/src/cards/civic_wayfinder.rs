//! Candidate-owned RAV Civic Wayfinder definition and ETB search binding.

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, Color, Effect, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-CIVIC-WAYFINDER",
        name: "Civic Wayfinder",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "enter-the-battlefield-private-basic-land-search",
            "selected-basic-land-reveal",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// Civic Wayfinder's mandatory ETB trigger searches for a basic land and
/// reveals the chosen card before putting it into its controller's hand.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-CIVIC-WAYFINDER",
        ability: TriggeredAbility {
            id: "etb-search-basic-land-to-hand",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SearchControllerLibrary {
                requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                    BasicLandType::Plains,
                    BasicLandType::Island,
                    BasicLandType::Swamp,
                    BasicLandType::Mountain,
                    BasicLandType::Forest,
                ])),
                destination: LibrarySearchDestination::Hand,
                selection: LibrarySearchSelection::PolicySubmitted {
                    may_fail_to_find: true,
                },
                reveal_selected: true,
            }],
        },
    }
}
