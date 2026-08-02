//! Red discovery contract for Pariah's Shield's persistent Equipment damage
//! replacement.  This must remain source-attached rather than becoming a
//! card-name branch or a finite temporary shield.

use cardbench_magic_engine::{
    AttachmentKind, ContinuousChange, ManaCost, TargetRequirement,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_attachment_bindings,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("{id} definition exists"))
}

#[test]
fn pariahs_shield_requires_persistent_equipped_damage_redirection() {
    let shield = definition("RAV-PARIAHS-SHIELD");
    assert_eq!(shield.mana_cost, ManaCost::new(5));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&shield.id),
        "Pariah's Shield must only be full fidelity with its persistent redirection"
    );
    assert!(shield
        .supported_rules
        .contains(&"equipment-all-damage-to-equipped-creature-to-controller"));

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == shield.id)
        .expect("Pariah's Shield attachment binding exists");
    assert_eq!(attachment.kind, AttachmentKind::Equipment);
    assert_eq!(attachment.target, TargetRequirement::ControlledCreature);
    assert_eq!(
        attachment.changes,
        vec![ContinuousChange::RedirectDamageToAttachmentController],
        "the replacement remains live only while this exact Equipment is attached"
    );

    let equip = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == shield.id && binding.ability.id == "equip-damage-redirection"
        })
        .expect("Pariah's Shield has its ordinary equip activation");
    assert_eq!(equip.ability.mana_cost, ManaCost::new(3));
    assert!(equip.ability.sorcery_speed);
    assert_eq!(equip.ability.targets, vec![TargetRequirement::ControlledCreature]);
}
