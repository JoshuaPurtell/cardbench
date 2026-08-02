//! Red regression for the public fixture-state provenance boundary.
//!
//! A life total can be changed through `Game::players` without a game action
//! or canonical event.  Its shape is otherwise ordinary, so the current
//! invariant audit cannot tell this fabricated state from one reached by an
//! omitted legal transition.

use cardbench_magic_engine::{CardDefinition, Game, PlayerId};

#[test]
#[ignore = "known bounded API provenance gap; requires breaking state-encapsulation migration"]
fn invariant_audit_rejects_shape_valid_external_life_transition() {
    let player = PlayerId(0);
    let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("fixture game initializes");
    game.begin_game().expect("game begins");
    game.validate_invariants()
        .expect("ordinary begin-game state is internally valid");

    let canonical_events = game.canonical_event_log();
    game.players[player.0].life = 13;

    let audit = game.validate_invariants();
    eprintln!(
        "fabricated life transition: audit={audit:?}; life={}; events_unchanged={}; events={canonical_events:?}",
        game.players[player.0].life,
        game.canonical_event_log() == canonical_events,
    );
    assert!(
        audit.is_err(),
        "a public life mutation with no transition receipt was accepted"
    );
}
