//! Red milestone for Peel from Reality's typed two-creature bounce.

use cardbench_magic_engine::{CastRequest, Color, Game, GameEvent, PlayerId, Target, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn peel_from_reality_requires_one_controlled_and_one_opponent_creature() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-PEEL-FROM-REALITY"),
        "Peel from Reality is not yet positively modeled; event_log={:?}",
        game.canonical_event_log()
    );
    let peel = game
        .add_card(PlayerId(0), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("Peel from Reality enters hand");
    let controlled = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature enters battlefield");
    let opponent = game
        .put_on_battlefield(PlayerId(1), "RAV-SNAPPING-DRAKE")
        .expect("opponent creature enters battlefield");
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("the instant's mana is available");
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: peel,
            targets: vec![Target::Permanent(controlled), Target::Permanent(opponent)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("typed pair targets cast");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and resolves");

    assert_eq!(game.zone_of(controlled), Some(Zone::Hand));
    assert_eq!(game.zone_of(opponent), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        } if *card == controlled
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        } if *card == opponent
    )));
    println!(
        "Peel from Reality event log: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("pair bounce preserves the game-state invariants");

    let mut wrong_side_game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    let wrong_side_peel = wrong_side_game
        .add_card(PlayerId(0), "RAV-PEEL-FROM-REALITY", Zone::Hand)
        .expect("Peel from Reality enters hand");
    let _controller_creature = wrong_side_game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled creature enters battlefield");
    let wrong_side_target = wrong_side_game
        .put_on_battlefield(PlayerId(1), "RAV-SNAPPING-DRAKE")
        .expect("opponent creature enters battlefield");
    wrong_side_game
        .grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("the instant's mana is available");
    wrong_side_game.clear_event_log();
    let error = wrong_side_game
        .cast_spell(
            PlayerId(0),
            CastRequest {
                card: wrong_side_peel,
                targets: vec![
                    Target::Permanent(wrong_side_target),
                    Target::Permanent(wrong_side_target),
                ],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .expect_err("the controlled-creature target role is mandatory");
    assert!(matches!(
        error,
        cardbench_magic_engine::RulesError::IllegalTarget(Target::Permanent(card))
            if card == wrong_side_target
    ));
    assert_eq!(wrong_side_game.zone_of(wrong_side_peel), Some(Zone::Hand));
    assert!(wrong_side_game.event_log.is_empty());
}
