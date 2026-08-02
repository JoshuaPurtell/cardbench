//! Red discovery contract for the generic "another creature leaves the
//! battlefield" trigger condition needed by Twilight Drover.

use cardbench_magic_engine::TriggerCondition;

#[test]
fn engine_exposes_another_creature_leaves_battlefield_trigger_condition() {
    assert_eq!(
        TriggerCondition::AnotherCreatureLeavesBattlefield,
        TriggerCondition::AnotherCreatureLeavesBattlefield
    );
}
