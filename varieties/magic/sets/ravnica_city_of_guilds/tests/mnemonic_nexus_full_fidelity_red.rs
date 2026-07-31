//! Red milestone for Mnemonic Nexus library recovery.
//!
//! The card is intentionally exercised through the real stack and zone APIs:
//! every player's graveyard must move into that player's library and then be
//! randomized by the engine's auditable shuffle boundary.

use cardbench_magic_engine::{CastRequest, Color, Game, PlayerId, Zone};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn mnemonic_nexus_moves_each_graveyard_into_its_library_and_shuffles() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game initializes");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-MNEMONIC-NEXUS"),
        "Mnemonic Nexus is not yet positively modeled; event_log={:?}",
        game.event_log
    );
    let nexus = game
        .add_card(PlayerId(0), "RAV-MNEMONIC-NEXUS", Zone::Hand)
        .expect("Mnemonic Nexus enters hand");
    let p0_graveyard = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Graveyard)
        .expect("player zero graveyard card enters setup");
    let p1_graveyard = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Graveyard)
        .expect("player one graveyard card enters setup");
    game.grant_mana(PlayerId(0), Color::Blue, 4)
        .expect("four mana pays the spell");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: nexus,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Mnemonic Nexus casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes and resolves");

    assert_eq!(game.zone_of(p0_graveyard), Some(Zone::Library));
    assert_eq!(game.zone_of(p1_graveyard), Some(Zone::Library));
    assert!(game
        .event_log
        .iter()
        .any(|event| matches!(event, cardbench_magic_engine::GameEvent::LibraryShuffled { player: PlayerId(0), .. })));
    assert!(game
        .event_log
        .iter()
        .any(|event| matches!(event, cardbench_magic_engine::GameEvent::LibraryShuffled { player: PlayerId(1), .. })));
    game.validate_invariants()
        .expect("library recovery preserves invariants");
}
