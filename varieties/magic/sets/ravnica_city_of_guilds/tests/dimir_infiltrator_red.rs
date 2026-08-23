//! Red regression for Dimir Infiltrator's absent executable RAV slice.
//!
//! Before the green milestone the public catalog has no definition for this
//! card, so setup must fail before a game can emit a gameplay receipt.

use cardbench_magic_engine::{Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn dimir_infiltrator_is_not_silently_catalog_only() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let result = game.add_card(PlayerId(0), "RAV-DIMIR-INFILTRATOR", Zone::Hand);
    println!(
        "Dimir Infiltrator red setup result: {result:?}; events_before={:?}",
        game.canonical_event_log()
    );
    let infiltrator = result.expect("Dimir Infiltrator definition exists");
    assert_eq!(game.zone_of(infiltrator), Some(Zone::Hand));
    assert!(game.event_log.is_empty());
}
