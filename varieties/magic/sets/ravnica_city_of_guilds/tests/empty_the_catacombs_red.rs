//! Regression for Empty the Catacombs' all-player graveyard return.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn empty_the_catacombs_returns_one_creature_card_per_graveyard_to_hand() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-EMPTY-THE-CATACOMBS")
        .expect("Empty the Catacombs definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"each-player-returns-creature-card-from-graveyard-to-hand")
    );

    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-EMPTY-THE-CATACOMBS", Zone::Hand)
        .expect("spell setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("first creature setup");
    let second = game
        .add_card(PlayerId(1), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("second creature setup");
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
    game.validate_invariants()
        .expect("all-player graveyard return stays valid");
}
