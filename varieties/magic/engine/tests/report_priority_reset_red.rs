//! Observational policy reports are ordinary non-pass priority actions.

use cardbench_magic_engine::{Game, PlayerId, PolicyAction, Step};

#[test]
fn weakness_report_resets_the_pass_sequence_before_the_reporter_passes() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(Vec::new(), 2).expect("fixture game initializes");
    let before_step = game.step;

    game.pass_priority(first)
        .expect("first player passes priority to the second");
    assert_eq!(game.priority, second);
    game.submit_policy_move(
        second,
        "test.report-priority-reset.v1",
        PolicyAction::ReportEngineWeakness {
            code: "test.priority-reset".to_owned(),
            detail: "a non-pass report must reopen the priority cycle".to_owned(),
        },
    )
    .expect("the priority holder reports an unsupported interaction");
    game.pass_priority(second)
        .expect("the reporter may pass after the observational action");

    eprintln!(
        "report-followed-by-pass state: step={:?}; priority={:?}; events={:?}",
        game.step,
        game.priority,
        game.canonical_event_log()
    );
    assert_eq!(
        game.step, before_step,
        "the reporter's pass must give the first player a response window rather than advance the step"
    );
    assert_eq!(
        game.priority, first,
        "a non-pass report resets the pass sequence and returns priority after the reporter passes"
    );
    assert_ne!(game.step, Step::BeginningOfCombat);
}
