//! Red regression for the reusable triggered-ability scheduling boundary.

use cardbench_magic_engine::{Effect, TargetRequirement, TriggerCondition};
use cardbench_magic_rav::rav_triggered_ability_bindings;

#[test]
fn belltower_damage_trigger_is_bound_through_the_generic_scheduler() {
    let ability = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-BELLTOWER-SPHINX")
        .expect("Belltower Sphinx damage trigger binding exists")
        .ability;

    assert_eq!(ability.condition, TriggerCondition::ReceivesDamage);
    assert_eq!(ability.targets, [TargetRequirement::Player]);
    assert_eq!(
        ability.effects,
        [Effect::MillTargetPlayerFromSourceDamage],
        "the source-damage amount must be materialized by the deferred trigger scheduler"
    );
}
