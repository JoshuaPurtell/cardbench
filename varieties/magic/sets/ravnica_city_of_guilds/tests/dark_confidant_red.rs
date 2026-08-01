use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn dark_confidant_requires_its_upkeep_reveal_and_mana_value_life_loss() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DARK-CONFIDANT")
        .expect("Dark Confidant definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into());
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(1)));
    assert!(definition
        .supported_rules
        .contains(&"beginning-of-upkeep-top-library-reveal-life-loss"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
