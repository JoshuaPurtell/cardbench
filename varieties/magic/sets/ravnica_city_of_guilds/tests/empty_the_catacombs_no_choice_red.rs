//! Regression that Empty the Catacombs does not ask players to select cards.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn empty_the_catacombs_returns_every_creature_and_ignores_noncreatures() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-EMPTY-THE-CATACOMBS", Zone::Hand)
        .expect("spell setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("first creature setup");
    let second = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Graveyard)
        .expect("second creature setup");
    let noncreature = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Graveyard)
        .expect("noncreature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("spell mana");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert_eq!(game.zone_of(first), Some(Zone::Hand));
    assert_eq!(game.zone_of(second), Some(Zone::Hand));
    assert_eq!(game.zone_of(noncreature), Some(Zone::Graveyard));
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_none()
    );
    assert!(
        game.canonical_event_log()
            .iter()
            .all(|event| !event.starts_with("DecisionOpened"))
    );
    game.validate_invariants()
        .expect("no-choice graveyard return stays valid");
}
