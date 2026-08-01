//! Red regression for Disembowel's missing chosen-X cast boundary.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, ManaPaymentSelection, PlayerId, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

#[test]
fn disembowel_pays_explicit_x_and_destroys_only_a_creature_within_that_bound() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let spell = game
        .add_card(PlayerId(0), "RAV-DISEMBOWEL", Zone::Hand)
        .expect("Disembowel setup");
    let target = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("target setup");
    game.grant_mana(PlayerId(0), Color::Black, 3)
        .expect("black mana for X=2 plus the colored symbol");

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Black, Color::Black],
            hybrid: vec![],
        },
    )
    .expect("chosen X pays Disembowel");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("spell resolves");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardDestroyed { source, card } if *source == spell && *card == target
    )));
    game.validate_invariants()
        .expect("chosen-X resolution preserves invariant state");
}
