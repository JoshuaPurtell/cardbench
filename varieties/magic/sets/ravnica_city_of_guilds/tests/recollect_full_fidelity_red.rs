use cardbench_magic_engine::{CardType, Color, Game, PlayerId, Target, Zone};
use cardbench_magic_rav::{SET_CODE, card_definitions};

#[test]
fn recollect_returns_a_targeted_card_from_its_controller_graveyard() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-RECOLLECT")
        .expect("Recollect must be present in the executable catalog");
    assert_eq!(definition.set_code, SET_CODE);
    assert_eq!(
        definition.colors,
        std::collections::BTreeSet::from([Color::Green])
    );
    assert_eq!(
        definition.card_types,
        std::collections::BTreeSet::from([CardType::Sorcery])
    );

    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-RECOLLECT", Zone::Hand)
        .expect("card is executable");
    let target = game
        .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .expect("graveyard target exists");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("test mana");
    game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast succeeds");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");
    assert_eq!(game.zone_of(target), Some(Zone::Hand));
}

#[test]
fn recollect_rejects_an_opponent_graveyard_target_atomically() {
    let mut game = Game::new(card_definitions(), 2).expect("catalog validates");
    let spell = game
        .add_card(PlayerId(0), "RAV-RECOLLECT", Zone::Hand)
        .expect("card is executable");
    let opponent_card = game
        .add_card(PlayerId(1), "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
        .expect("opponent graveyard target exists");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("test mana");
    let result = game.cast_spell(
        PlayerId(0),
        cardbench_magic_engine::CastRequest {
            card: spell,
            targets: vec![Target::Permanent(opponent_card)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(
        result.is_err(),
        "Recollect cannot target an opponent's graveyard"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(opponent_card), Some(Zone::Graveyard));
    assert!(game.event_log.is_empty());
}
