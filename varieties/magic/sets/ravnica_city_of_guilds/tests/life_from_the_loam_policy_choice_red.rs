//! Red regression for Life from the Loam's public graveyard-selection gap.
//!
//! The printed "up to three" instruction requires the resolving controller
//! to choose the cards.  Public zone order is not a legal substitute for that
//! choice, even when there are exactly three candidates.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn life_from_the_loam_waits_for_its_controllers_public_land_selection() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let loam = game
        .add_card(PlayerId(0), "RAV-LIFE-FROM-THE-LOAM", Zone::Hand)
        .expect("Life from the Loam setup");
    let lands = [
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Graveyard)
            .expect("Forest setup"),
        game.add_card(PlayerId(0), "RAV-MOUNTAIN", Zone::Graveyard)
            .expect("Mountain setup"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Graveyard)
            .expect("Island setup"),
    ];
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("mana setup");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: loam,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Life from the Loam casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .pending_decision
            .is_some(),
        "Life from the Loam must suspend for an explicit public land choice; trace: {:?}",
        game.canonical_event_log()
    );
    assert!(
        lands
            .iter()
            .all(|card| game.zone_of(*card) == Some(Zone::Graveyard)),
        "no selected land may move before its controller answers the choice"
    );
}
