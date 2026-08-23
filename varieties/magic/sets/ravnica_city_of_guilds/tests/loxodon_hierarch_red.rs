//! Red fidelity contract for Loxodon Hierarch's trigger and sacrifice ability.

use cardbench_magic_engine::Effect;
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn loxodon_hierarch_has_its_complete_entry_and_team_regeneration_rules() {
    let hierarchy = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-LOXODON-HIERARCH")
        .expect("Loxodon Hierarch definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&hierarchy.id),
        "Loxodon Hierarch cannot be complete while its entry trigger and team regeneration are absent"
    );
    assert!(hierarchy.supported_rules.contains(&"etb-gain-four-life"));
    assert!(
        hierarchy
            .supported_rules
            .contains(&"sacrifice-source-regenerate-controller-creatures")
    );
    assert!(rav_triggered_ability_bindings().into_iter().any(|binding| {
        binding.card_definition == hierarchy.id
            && binding.ability.id == "etb-gain-four-life"
            && binding.ability.effects == [Effect::GainLifeController { amount: 4 }]
    }));
    assert!(rav_activated_ability_bindings().into_iter().any(|binding| {
        binding.card_definition == hierarchy.id
            && binding.ability.id == "sacrifice-source-regenerate-controller-creatures"
            && binding.ability.sacrifice_source
    }));
}
