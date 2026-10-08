//! Red discovery contract for Bramble Elemental's Aura-attachment trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Effect, ManaCost, TokenSpec, TriggerCondition};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn bramble_elemental_requires_optional_two_saprolings_when_aura_attaches_to_it() {
    let bramble = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BRAMBLE-ELEMENTAL")
        .expect("Bramble Elemental definition exists");

    assert_eq!(bramble.name, "Bramble Elemental");
    assert_eq!(
        bramble.mana_cost,
        ManaCost::with_colors(3, [Color::Green, Color::Green])
    );
    assert_eq!(bramble.colors, BTreeSet::from([Color::Green]));
    assert_eq!(bramble.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((bramble.power, bramble.toughness), (Some(4), Some(4)));

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-BRAMBLE-ELEMENTAL")
        .expect("Bramble Elemental Aura-attachment trigger binding exists");
    assert_eq!(trigger.ability.id, "aura-attached-create-two-saprolings");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::AuraBecomesAttachedToSource
    );
    assert!(trigger.ability.optional);
    assert!(trigger.ability.targets.is_empty());
    assert_eq!(
        trigger.ability.effects,
        vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 2,
        }]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&bramble.id),
        "the exact-source may-trigger and two Saprolings complete this card"
    );
}
