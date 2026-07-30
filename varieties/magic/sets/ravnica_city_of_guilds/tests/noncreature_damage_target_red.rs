use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Target, Zone};

#[test]
fn lightning_helix_rejects_a_noncreature_permanent_target() {
    let mut game = Game::new(cardbench_magic_rav::card_definitions(), 2)
        .expect("two-player RAV game initializes");
    let helix = game
        .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .expect("Lightning Helix enters the caster's hand");
    let plains = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Battlefield)
        .expect("a noncreature land enters the opponent's battlefield");
    game.grant_mana(PlayerId(0), Color::White, 1)
        .expect("white mana is granted");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana is granted");

    let before_events = game.canonical_event_log();
    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Permanent(plains)],
            convoke: vec![],
        },
    );

    assert!(
        result.is_err(),
        "Lightning Helix accepted a land target; result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected target must not enter the stack or write a cast receipt"
    );
}
