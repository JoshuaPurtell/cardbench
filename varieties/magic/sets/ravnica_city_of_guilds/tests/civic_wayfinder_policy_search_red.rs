//! Red regression for Civic Wayfinder's controller-submitted land search.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandType, Effect, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, TriggerCondition,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, rav_triggered_ability_bindings};

#[test]
fn civic_wayfinder_requires_a_private_policy_submitted_basic_land_search() {
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-CIVIC-WAYFINDER")
        .expect("Civic Wayfinder ETB binding exists");
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert_eq!(
        binding.ability.effects,
        vec![Effect::SearchControllerLibrary {
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
        "the controller must own the private optional search selection"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CIVIC-WAYFINDER"),
        "the policy-owned search completes Civic Wayfinder's represented behavior"
    );
}
