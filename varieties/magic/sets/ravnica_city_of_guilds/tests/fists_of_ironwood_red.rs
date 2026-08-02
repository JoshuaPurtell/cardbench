//! Red discovery contract for Fists of Ironwood's complete Aura behavior.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
    TokenSpec, TriggerCondition,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn fists_of_ironwood_requires_creature_aura_trample_and_etb_saprolings() {
    let fists = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FISTS-OF-IRONWOOD")
        .expect("Fists of Ironwood definition exists");

    assert_eq!(fists.name, "Fists of Ironwood");
    assert_eq!(fists.mana_cost, ManaCost::with_colors(1, [Color::Green]));
    assert_eq!(fists.colors, BTreeSet::from([Color::Green]));
    assert_eq!(fists.card_types, BTreeSet::from([CardType::Enchantment]));
    assert_eq!(fists.power, None);
    assert_eq!(fists.toughness, None);
    assert_eq!(fists.keywords, []);
    assert_eq!(
        fists.effects,
        [Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::Trample)],
        }]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&fists.id));
    assert!(
        fists
            .supported_rules
            .contains(&"aura-enchant-creature-trample-etb-two-saprolings")
    );

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-FISTS-OF-IRONWOOD")
        .expect("Fists of Ironwood Saproling trigger exists");
    assert_eq!(trigger.ability.id, "etb-two-saprolings");
    assert_eq!(trigger.ability.condition, TriggerCondition::EntersBattlefield);
    assert_eq!(
        trigger.ability.effects,
        [Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 2,
        }]
    );
}
