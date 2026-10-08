//! Registry contract for Galvanic Arc's ETB damage and attached First Strike.

use cardbench_magic_engine::{
    AttachmentKind, ContinuousChange, Effect, Keyword, TargetRequirement, TriggerCondition,
};
use cardbench_magic_rav::{rav_attachment_bindings, rav_triggered_ability_bindings};

#[test]
fn galvanic_arc_uses_a_targeted_etb_trigger_and_a_first_strike_aura_layer() {
    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-GALVANIC-ARC")
        .expect("Galvanic Arc ETB trigger binding exists");
    assert_eq!(trigger.ability.id, "etb-deal-three");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert!(!trigger.ability.optional);
    assert_eq!(
        trigger.ability.targets,
        vec![TargetRequirement::PlayerOrCreature]
    );
    assert_eq!(
        trigger.ability.effects,
        vec![Effect::DealDamage {
            amount: 3,
            target: TargetRequirement::PlayerOrCreature,
        }]
    );

    let attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-GALVANIC-ARC")
        .expect("Galvanic Arc Aura binding exists");
    assert_eq!(attachment.kind, AttachmentKind::Aura);
    assert_eq!(attachment.target, TargetRequirement::Creature);
    assert_eq!(
        attachment.changes,
        vec![ContinuousChange::AddKeyword(Keyword::FirstStrike)]
    );
    assert!(attachment.granted_activated_abilities.is_empty());
}
