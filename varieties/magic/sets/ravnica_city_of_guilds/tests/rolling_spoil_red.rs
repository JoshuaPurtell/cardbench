use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn rolling_spoil_requires_a_spent_black_global_modifier_branch() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-ROLLING-SPOIL")
        .expect("Rolling Spoil definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::Green])
    );
    assert_eq!(definition.colors, [Color::Green].into());
    assert_eq!(definition.card_types, [CardType::Sorcery].into());
    assert!(
        definition
            .supported_rules
            .contains(&"spent-black-global-minus-one")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
