use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn scion_of_the_wild_requires_live_controller_creature_count_characteristics() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCION-OF-THE-WILD")
        .expect("Scion of the Wild definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Green, Color::Green])
    );
    assert_eq!(definition.colors, [Color::Green].into());
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!((definition.power, definition.toughness), (Some(0), Some(0)));
    assert!(
        definition
            .supported_rules
            .contains(&"dynamic-controlled-creature-count")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
