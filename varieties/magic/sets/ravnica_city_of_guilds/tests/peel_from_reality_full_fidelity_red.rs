//! Red milestone for Peel from Reality's typed two-creature bounce.

use cardbench_magic_engine::Game;
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn peel_from_reality_requires_one_controlled_and_one_opponent_creature() {
    let game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-PEEL-FROM-REALITY"),
        "Peel from Reality is not yet positively modeled; event_log={:?}",
        game.canonical_event_log()
    );
}
