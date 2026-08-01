//! Red discovery regression for Ivy Dancer's targeted Forestwalk activation.

use cardbench_magic_engine::{Effect, Keyword, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, RAV_FULL_FIDELITY_DEFINITION_IDS,
};

#[test]
fn ivy_dancer_requires_its_targeted_tap_forestwalk_activation() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-IVY-DANCER")
        .expect("Ivy Dancer definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "the targeted temporary Forestwalk activation is required"
    );
    assert!(definition
        .supported_rules
        .contains(&"tap-target-creature-grant-forestwalk"));

    let ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-IVY-DANCER"
                && binding.ability.id == "tap-target-creature-grant-forestwalk"
        })
        .expect("Ivy Dancer targeted Forestwalk binding exists");
    assert_eq!(ability.ability.mana_cost, ManaCost::new(0));
    assert!(ability.ability.tap_cost);
    assert_eq!(ability.ability.targets, [TargetRequirement::Creature]);
    assert_eq!(
        ability.ability.effects,
        [Effect::ModifyTargetKeywordUntilEndOfTurn {
            keyword: Keyword::Forestwalk,
        }]
    );
}
