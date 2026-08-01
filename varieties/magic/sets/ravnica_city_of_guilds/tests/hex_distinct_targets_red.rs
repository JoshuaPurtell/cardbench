use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn hex_requires_six_distinct_creature_targets() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HEX")
        .expect("Hex definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(4, [Color::Black, Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into());
    assert_eq!(definition.card_types, [CardType::Sorcery].into());
    assert!(definition
        .supported_rules
        .contains(&"six-distinct-creature-destruction"));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
