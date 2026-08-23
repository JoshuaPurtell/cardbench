//! Red regression for Drake Familiar's target-bearing enter-the-battlefield ability.
//!
//! The ordinary target-bearing trigger boundary must retain an enchantment
//! target through priority and move that target to its owner's hand only at
//! stack resolution.

use cardbench_magic_engine::{Keyword, TargetRequirement, TriggerCondition};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

const DRAKE_FAMILIAR: &str = "RAV-DRAKE-FAMILIAR";

#[test]
fn drake_familiar_requires_a_targeted_enchantment_return_etb() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == DRAKE_FAMILIAR)
        .expect("Drake Familiar definition exists");
    assert_eq!(definition.keywords, vec![Keyword::Flying]);
    assert!(
        definition
            .supported_rules
            .contains(&"etb-target-enchantment-owner-hand"),
        "the entry trigger is part of Drake Familiar's represented behavior"
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Drake Familiar cannot be full fidelity without its target-bearing ETB"
    );

    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == DRAKE_FAMILIAR)
        .expect("Drake Familiar has a target-enchantment ETB binding");
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert_eq!(
        binding.ability.targets,
        vec![TargetRequirement::Enchantment]
    );
    assert_eq!(
        binding
            .ability
            .effects
            .iter()
            .flat_map(|effect| effect.target_requirements().into_iter().flatten())
            .collect::<Vec<_>>(),
        vec![TargetRequirement::Enchantment],
        "the trigger retains its enchantment target as a normal stack target"
    );
}
