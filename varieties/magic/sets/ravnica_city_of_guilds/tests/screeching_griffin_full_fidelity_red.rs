//! Red discovery contract for Screeching Griffin's activated evasion ability.
//!
//! This test intentionally starts as a failing milestone: the card is present
//! as a Flying-compatible chassis, but its `{R}` ability that prevents one
//! chosen creature from blocking this source is not yet bound to the stack.

use cardbench_magic_engine::{CardType, Color};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, RAV_FULL_FIDELITY_DEFINITION_IDS,
};

#[test]
fn screeching_griffin_requires_its_activated_ability_for_full_fidelity() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");
    assert_eq!(griffin.colors, [Color::White].into_iter().collect());
    assert_eq!(griffin.card_types, [CardType::Creature].into_iter().collect());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
    assert!(griffin
        .supported_rules
        .contains(&"activated-prevent-target-blocking-source"));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-SCREECHING-GRIFFIN"
            && binding.ability.id == "prevent-target-blocking-griffin"
    }));
}
