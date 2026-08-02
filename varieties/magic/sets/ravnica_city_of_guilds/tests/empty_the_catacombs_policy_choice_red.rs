//! Red regression for Empty the Catacombs' required public graveyard choices.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn empty_the_catacombs_waits_for_each_players_public_graveyard_choice() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture constructs");
    let spell = game
        .add_card(PlayerId(0), "RAV-EMPTY-THE-CATACOMBS", Zone::Hand)
        .expect("spell setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("first creature setup");
    let selected = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Graveyard)
        .expect("selected creature setup");
    let opponent_creature = game
        .add_card(PlayerId(1), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("opponent creature setup");
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

    // Each player chooses a creature card in their own public graveyard as
    // the spell resolves. Resolution must pause at that policy boundary; it
    // cannot silently use insertion order and move a card before the owner
    // can choose it.
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_some(),
        "Empty the Catacombs must open a public graveyard-choice decision"
    );
    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(selected), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(opponent_creature), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("a pending public-zone choice is an invariant-valid state");
}
