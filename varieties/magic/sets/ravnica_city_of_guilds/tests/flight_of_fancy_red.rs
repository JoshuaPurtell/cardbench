//! Red discovery contract for Flight of Fancy's Aura and enter-the-battlefield draw.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, Effect, Keyword, ManaCost, TargetRequirement,
    TriggerCondition,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_triggered_ability_bindings,
};

#[test]
fn flight_of_fancy_requires_creature_aura_flying_and_two_card_etb_draw() {
    let flight = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FLIGHT-OF-FANCY")
        .expect("Flight of Fancy definition exists");

    assert_eq!(flight.name, "Flight of Fancy");
    assert_eq!(flight.mana_cost, ManaCost::with_colors(3, [Color::Blue]));
    assert_eq!(flight.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(flight.card_types, BTreeSet::from([CardType::Enchantment]));
    assert_eq!(flight.power, None);
    assert_eq!(flight.toughness, None);
    assert_eq!(flight.keywords, []);
    assert_eq!(
        flight.effects,
        [Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::Flying)],
        }]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&flight.id));
    assert!(
        flight
            .supported_rules
            .contains(&"aura-enchant-creature-flying-etb-draw-two")
    );

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == "RAV-FLIGHT-OF-FANCY")
        .expect("Flight of Fancy draw trigger exists");
    assert_eq!(trigger.ability.id, "etb-draw-two");
    assert_eq!(
        trigger.ability.condition,
        TriggerCondition::EntersBattlefield
    );
    assert_eq!(
        trigger.ability.effects,
        [Effect::DrawController, Effect::DrawController]
    );
}
