//! Red regression for the source-only Firemane Angel card definition.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, Keyword, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn firemane_angel_has_its_exact_static_chassis_without_claiming_unported_text() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FIREMANE-ANGEL")
        .expect("Firemane Angel definition exists");

    assert_eq!(definition.name, "Firemane Angel");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::White, Color::White])
    );
    assert_eq!(
        definition.colors,
        [Color::Red, Color::White].into_iter().collect()
    );
    assert_eq!(
        definition.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert_eq!(definition.power, Some(4));
    assert_eq!(definition.toughness, Some(3));
    assert_eq!(definition.keywords, [Keyword::Flying, Keyword::FirstStrike]);
    assert_eq!(
        definition.supported_rules,
        [
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "first-strike",
        ]
    );
}

#[test]
fn firemane_angel_casts_as_a_flying_first_striker() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let angel = game
        .add_card(PlayerId(0), "RAV-FIREMANE-ANGEL", Zone::Hand)
        .expect("Angel setup");
    game.grant_mana(PlayerId(0), Color::Red, 4)
        .expect("red mana");
    game.grant_mana(PlayerId(0), Color::White, 5)
        .expect("white mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: angel,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Angel casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("Angel resolves");

    let characteristics = game.characteristics(angel).expect("Angel is live");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(3));
    assert!(characteristics.keywords.contains(&Keyword::Flying));
    assert!(characteristics.keywords.contains(&Keyword::FirstStrike));
    game.validate_invariants()
        .expect("Firemane Angel's bounded chassis preserves invariants");
}
