//! Red regression for Belltower Sphinx's source-controller damage trigger.

use cardbench_magic_engine::{Effect, TargetRequirement, TriggerCondition};
use cardbench_magic_rav::rav_triggered_ability_bindings;

#[test]
fn belltower_damage_trigger_mills_damage_source_controller_without_target_choice() {
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx damage trigger binding exists")
        .ability;

    assert_eq!(ability.condition, TriggerCondition::ReceivesDamage);
    assert!(ability.targets.is_empty(), "the trigger has no player target");
    assert_eq!(
        ability.effects,
        [Effect::MillSourceControllerFromSourceDamage],
        "damage must mill the controller of the source that dealt it"
    );
    assert_eq!(
        ability.targets,
        [] as [TargetRequirement; 0],
        "the source controller is derived, not selected"
    );
}
