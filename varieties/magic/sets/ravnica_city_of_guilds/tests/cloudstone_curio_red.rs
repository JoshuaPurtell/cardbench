//! Red regression for Cloudstone Curio's controller-choice bounce trigger.
//!
//! Cloudstone Curio is deliberately a non-targeting trigger: after a
//! nonartifact permanent enters under its controller, the controller may
//! choose another controlled permanent sharing a card type and return it to
//! its owner's hand.  The green repair must retain that entering-permanent
//! identity through stack resolution instead of turning the choice into a
//! generic or opponent-facing target.

use cardbench_magic_engine::{CardType, ManaCost, TriggerCondition};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn cloudstone_curio_has_the_full_colorless_etb_choice_contract() {
    let curio = definition("RAV-CLOUDSTONE-CURIO");
    assert_eq!(curio.mana_cost, ManaCost::new(3));
    assert_eq!(curio.card_types, [CardType::Artifact].into_iter().collect());
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&curio.id),
        "Cloudstone Curio needs the complete optional, source-relative ETB bounce path"
    );
    assert!(
        curio
            .supported_rules
            .contains(&"controlled-nonartifact-etb-may-bounce-another-sharing-card-type")
    );
}

#[test]
fn cloudstone_curio_binds_a_controller_scoped_nonartifact_entry_trigger() {
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-CLOUDSTONE-CURIO")
        .expect("Cloudstone Curio trigger binding exists");
    assert_eq!(
        binding.ability.condition,
        TriggerCondition::ControlledNonartifactPermanentEntersBattlefield
    );
    assert!(binding.ability.optional, "the return remains a may choice");
    assert!(
        binding.ability.targets.is_empty(),
        "the printed ability does not target"
    );
}
