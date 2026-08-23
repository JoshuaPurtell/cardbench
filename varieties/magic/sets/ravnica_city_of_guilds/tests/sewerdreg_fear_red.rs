//! Red regression for Sewerdreg's static Fear evasion.
//!
//! This is intentionally a behavior-first probe: a nonartifact, nonblack
//! creature must not be able to block the attacker. The current bounded
//! definition has no Fear keyword, so the declaration is expected to be
//! accepted until the engine/card slice is fixed.

use cardbench_magic_engine::{CombatBlock, Game, PlayerId, RulesError, Step, Zone};
use cardbench_magic_rav::card_definitions;

#[test]
fn sewerdreg_rejects_a_nonblack_nonartifact_blocker() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let sewerdreg = game
        .add_card(PlayerId(0), "RAV-SEWERDREG", Zone::Battlefield)
        .expect("Sewerdreg begins on battlefield");
    let blocker = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("ground blocker begins on battlefield");
    game.set_entered_turn_for_setup(sewerdreg, 0)
        .expect("Sewerdreg predates the measured turn");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker predates the measured turn");
    game.begin_game().expect("fixture starts game");
    for _ in 0..4 {
        game.pass_priority(PlayerId(0))
            .expect("active player passes toward combat");
        game.pass_priority(PlayerId(1))
            .expect("opponent passes toward combat");
    }
    assert_eq!(game.step, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[sewerdreg])
        .expect("Sewerdreg may attack");
    game.pass_priority(PlayerId(0))
        .expect("attacking player passes after declaring attackers");
    game.pass_priority(PlayerId(1))
        .expect("defending player passes into blocker declaration");
    assert_eq!(game.step, Step::DeclareBlockers);
    let events_before_rejection = game.event_log.clone();

    assert_eq!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker: sewerdreg,
                blocker,
            }],
        ),
        Err(RulesError::IllegalAction(
            "fear attacker can be blocked only by black or artifact creatures",
        ))
    );
    assert_eq!(game.event_log, events_before_rejection);
    game.validate_invariants()
        .expect("rejected Fear block preserves engine invariants");
}
