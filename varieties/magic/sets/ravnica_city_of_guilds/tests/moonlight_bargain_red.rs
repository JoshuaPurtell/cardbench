//! Red regression for Moonlight Bargain's missing RAV execution slice.
//!
//! The probe intentionally fails before any gameplay mutation: the catalog
//! entry is currently not executable, so no event should be emitted.

use cardbench_magic_engine::{Game, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn moonlight_bargain_is_not_silently_catalog_only() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    let result = game.add_card(PlayerId(0), "RAV-MOONLIGHT-BARGAIN", Zone::Hand);
    println!(
        "Moonlight Bargain red setup result: {result:?}; events_before={:?}",
        game.canonical_event_log()
    );
    let bargain = result.expect("Moonlight Bargain definition exists");
    assert_eq!(game.zone_of(bargain), Some(Zone::Hand));
    assert!(game.event_log.is_empty());
}
