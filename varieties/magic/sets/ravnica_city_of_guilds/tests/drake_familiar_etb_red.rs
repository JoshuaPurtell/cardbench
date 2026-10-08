//! Red regression for Drake Familiar's target-free return-or-sacrifice trigger.

use cardbench_magic_engine::{Effect, Keyword, TriggerCondition};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

const DRAKE_FAMILIAR: &str = "RAV-DRAKE-FAMILIAR";
const ABILITY_ID: &str = "etb-return-any-enchantment-or-sacrifice-source";

#[test]
fn drake_familiar_requires_a_target_free_any_enchantment_return_or_sacrifice_etb() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == DRAKE_FAMILIAR)
        .expect("Drake Familiar definition exists");
    assert_eq!(definition.keywords, vec![Keyword::Flying]);
    assert!(
        definition
            .supported_rules
            .contains(&"etb-return-any-enchantment-or-sacrifice-source")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));

    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == DRAKE_FAMILIAR)
        .expect("Drake Familiar has a candidate-owned ETB binding");
    assert_eq!(binding.ability.id, ABILITY_ID);
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert!(
        binding.ability.targets.is_empty(),
        "the ability is not targeted"
    );
    assert!(!binding.ability.optional, "the trigger itself is mandatory");
    assert_eq!(
        binding.ability.effects,
        vec![Effect::ReturnAnyEnchantmentOrSacrificeSource]
    );
}
