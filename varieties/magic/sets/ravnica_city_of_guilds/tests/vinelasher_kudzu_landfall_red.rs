//! Red discovery contract for the source lane's unmerged Vinelasher landfall.

use cardbench_magic_engine::{Effect, TriggerCondition};
use cardbench_magic_rav::rav_triggered_ability_bindings;

#[test]
fn vinelasher_kudzu_registers_its_controller_land_entry_counter_trigger() {
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-VINELASHER-KUDZU")
        .expect("Vinelasher Kudzu landfall trigger exists");

    assert_eq!(binding.ability.condition, TriggerCondition::LandEntersBattlefield);
    assert_eq!(binding.ability.targets, []);
    assert_eq!(binding.ability.effects, [Effect::AddPlusOneCounterToSource]);
}
