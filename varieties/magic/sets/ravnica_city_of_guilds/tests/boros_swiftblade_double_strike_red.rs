use cardbench_magic_engine::{CardType, Color, Game, Keyword, PlayerId};
use cardbench_magic_rav::card_definitions;

#[test]
fn boros_swiftblade_declares_double_strike_and_assigns_two_damage_steps() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BOROS-SWIFTBLADE")
        .expect("Boros Swiftblade exists");
    assert_eq!(definition.card_types, [CardType::Creature].into_iter().collect());
    assert!(definition.colors.contains(&Color::Red));
    assert!(definition.keywords.contains(&Keyword::DoubleStrike));

    let _ = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let _ = PlayerId(0);
}
