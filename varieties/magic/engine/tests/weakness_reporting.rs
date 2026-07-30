//! Fail-loud coverage for unsupported interactions discovered by a policy run.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, PolicyAction, PolicyMoveKind};

#[test]
fn accepted_weakness_report_is_auditable_and_does_not_mutate_game_flow() {
    let mut game = Game::new(Vec::new(), 2).expect("two-player game initializes");
    let before = (game.turn, game.step, game.priority, game.stack.len());
    game.clear_event_log();

    game.submit_policy_move(
        PlayerId(0),
        "test.weakness-reporter.v1",
        PolicyAction::ReportEngineWeakness {
            code: "combat.multiple_blockers".to_owned(),
            detail: "public fixture requires two blockers on one attacker".to_owned(),
        },
    )
    .expect("priority holder can submit a weakness report");

    assert_eq!(
        (game.turn, game.step, game.priority, game.stack.len()),
        before,
        "reporting does not pretend the unsupported interaction resolved"
    );
    game.validate_invariants()
        .expect("observational report preserves all invariants");
    assert!(matches!(
        &game.event_log[..],
        [
            GameEvent::EngineWeaknessRevealed { player: PlayerId(0), code, detail },
            GameEvent::PolicyMoveSubmitted { player: PlayerId(0), policy, kind: PolicyMoveKind::ReportEngineWeakness },
        ] if code == "combat.multiple_blockers"
            && detail == "public fixture requires two blockers on one attacker"
            && policy == "test.weakness-reporter.v1"
    ));
}
