use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn netherborn_phalanx_requires_its_dynamic_opponent_creature_count_etb_trigger() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NETHERBORN-PHALANX")
        .expect("Netherborn Phalanx definition exists");

    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(5, [Color::Black])
    );
    assert_eq!(definition.colors, [Color::Black].into());
    assert_eq!(definition.card_types, [CardType::Creature].into());
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(4)));
    assert!(
        definition
            .keywords
            .contains(&Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            )))
    );
    assert!(
        definition
            .supported_rules
            .contains(&"enter-the-battlefield-opponent-creature-count-life-loss")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
}
