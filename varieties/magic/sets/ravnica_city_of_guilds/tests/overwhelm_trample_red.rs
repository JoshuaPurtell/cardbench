use cardbench_magic_engine::{CardType, CastRequest, Color, Game, Keyword, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn overwhelm_grants_trample_until_end_of_turn() {
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLIATH-SPIDER")
        .expect("controlled creature");
    let spell = game
        .add_card(PlayerId(0), "RAV-OVERWHELM", Zone::Hand)
        .expect("Overwhelm is executable");
    game.grant_mana(PlayerId(0), Color::Green, 7)
        .expect("test mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast succeeds");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");
    let characteristics = game.characteristics(creature).expect("characteristics");
    assert!(characteristics.keywords.contains(&Keyword::Trample));
    assert_eq!(
        characteristics.card_types,
        std::collections::BTreeSet::from([CardType::Creature])
    );
}
