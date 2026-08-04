//! Milestone 1: no stale, misattributed, or duplicated command is admitted.
//!
//! Every test here asserts a *refusal*. The invariant being defended is that
//! a command which does not exactly match the open request never reaches the
//! rules engine, so it cannot produce a partial mutation no matter what the
//! engine would have done with it.

mod support;

use cardbench_magic_protocol::{
    ActionRequestId, ClientCommandId, CommandKind, CommandLedger, CommandReceipt, CommandResult,
    DecisionRef, GameCommand, MatchId, ProtocolError, SchemaVersion, SeatId, SubmissionContext,
    admit, validate_envelope, validate_schema,
};
use support::{SEAT_A, SEAT_B, SEAT_C, match_id, pass, priority_request, revision};

const SEATS: [SeatId; 2] = [SEAT_A, SEAT_B];

#[test]
fn a_matching_command_is_admitted() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let envelope = pass(&request, "cmd-1");
    assert_eq!(validate_envelope(&envelope, &context), Ok(()));
    assert!(admit(&envelope, &context, &CommandLedger::new()).is_none());
}

#[test]
fn an_incompatible_major_schema_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.schema = SchemaVersion::new(2, 0, 0);
    assert!(matches!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::SchemaMismatch { .. })
    ));
}

/// A minor or patch difference must stay compatible, or every additive schema
/// change becomes a flag day for clients.
#[test]
fn a_newer_minor_schema_is_accepted() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.schema = SchemaVersion::new(
        cardbench_magic_protocol::PROTOCOL_SCHEMA_VERSION.major,
        99,
        3,
    );
    assert_eq!(validate_envelope(&envelope, &context), Ok(()));
    assert_eq!(validate_schema(envelope.schema), Ok(()));
}

#[test]
fn a_command_for_another_match_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.match_id = MatchId::new("some-other-match");
    assert_eq!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::MatchMismatch {
            expected: match_id(),
            actual: MatchId::new("some-other-match"),
        })
    );
}

#[test]
fn a_command_from_a_seat_that_does_not_exist_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.seat = SEAT_C;
    assert_eq!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::UnknownSeat { seat: SEAT_C })
    );
}

/// The out-of-turn case. A seat that exists but was not asked to act cannot
/// act, even when the command it submits would be legal for the seat that was
/// asked.
#[test]
fn a_command_from_the_wrong_seat_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.seat = SEAT_B;
    assert_eq!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::WrongSeat {
            expected: SEAT_A,
            actual: SEAT_B,
        })
    );
}

#[test]
fn a_command_answering_a_closed_request_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let envelope = pass(&request, "cmd-1");

    // The match moved on: a later request is now open at a later revision.
    let later = priority_request(SEAT_A, revision(6), 13);
    let context = SubmissionContext::from_request(&later, &SEATS);

    assert_eq!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::StaleRequest {
            observed: ActionRequestId(12),
            current: ActionRequestId(13),
        })
    );
}

/// The subtle case: the same request id, but the state moved underneath it.
/// Without the revision check this would be indistinguishable from a fresh
/// command.
#[test]
fn a_command_observing_an_older_revision_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let mut envelope = pass(&request, "cmd-1");
    envelope.observed_revision = revision(4);
    let context = SubmissionContext::from_request(&request, &SEATS);
    assert_eq!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::StaleRevision {
            observed: revision(4),
            current: revision(5),
        })
    );
}

/// Same sequence, different integrity digest. A client that somehow observed a
/// rebuilt state at the same sequence must be refused rather than allowed to
/// act on a state that no longer exists.
#[test]
fn a_command_observing_a_divergent_state_at_the_same_sequence_is_refused() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let mut envelope = pass(&request, "cmd-1");
    envelope.observed_revision =
        cardbench_magic_protocol::StateRevision::new(request.revision.sequence, 0x1234);
    let context = SubmissionContext::from_request(&request, &SEATS);
    assert!(matches!(
        validate_envelope(&envelope, &context),
        Err(ProtocolError::StaleRevision { .. })
    ));
}

/// A decision identity is checked independently of the request identity: the
/// engine's decision ids are the stale-safety mechanism for mandatory choices
/// and a command must not satisfy a different decision of the same shape.
#[test]
fn a_command_echoing_the_wrong_decision_is_refused() {
    let mut request = priority_request(SEAT_A, revision(5), 12);
    request.kind = cardbench_magic_protocol::ActionRequestKind::Draw;
    request.decision = Some(prompt(DecisionRef(7)));
    let context = SubmissionContext::from_request(&request, &SEATS);

    let wrong = request.reply(
        ClientCommandId::new("cmd-1"),
        GameCommand::Draw {
            decision: DecisionRef(8),
            dredge: None,
        },
    );
    assert_eq!(
        validate_envelope(&wrong, &context),
        Err(ProtocolError::StaleDecision {
            observed: Some(DecisionRef(8)),
            expected: Some(DecisionRef(7)),
        })
    );

    let right = request.reply(
        ClientCommandId::new("cmd-2"),
        GameCommand::Draw {
            decision: DecisionRef(7),
            dredge: None,
        },
    );
    assert_eq!(validate_envelope(&right, &context), Ok(()));
}

/// The converse: a command that answers no decision cannot satisfy a request
/// that opened one, and vice versa.
#[test]
fn decision_presence_must_match_the_request() {
    let mut decision_request = priority_request(SEAT_A, revision(5), 12);
    decision_request.kind = cardbench_magic_protocol::ActionRequestKind::Draw;
    decision_request.decision = Some(prompt(DecisionRef(7)));
    let decision_context = SubmissionContext::from_request(&decision_request, &SEATS);
    assert_eq!(
        validate_envelope(&pass(&decision_request, "cmd-1"), &decision_context),
        Err(ProtocolError::StaleDecision {
            observed: None,
            expected: Some(DecisionRef(7)),
        })
    );

    let priority = priority_request(SEAT_A, revision(5), 12);
    let priority_context = SubmissionContext::from_request(&priority, &SEATS);
    let decision_answer = priority.reply(
        ClientCommandId::new("cmd-2"),
        GameCommand::Draw {
            decision: DecisionRef(7),
            dredge: None,
        },
    );
    assert_eq!(
        validate_envelope(&decision_answer, &priority_context),
        Err(ProtocolError::StaleDecision {
            observed: Some(DecisionRef(7)),
            expected: None,
        })
    );
}

/// Rejection reports the current revision, unchanged. A caller that mutated
/// state before rejecting could not satisfy this.
#[test]
fn rejection_reports_the_current_revision_and_nothing_else() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let mut envelope = pass(&request, "cmd-1");
    envelope.observed_revision = revision(1);

    let result = admit(&envelope, &context, &CommandLedger::new()).expect("must be refused");
    match &result {
        CommandResult::Rejected {
            revision: current, ..
        } => {
            assert_eq!(*current, request.revision);
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
    assert!(result.is_inert());
    assert!(!result.is_accepted());
}

/// Reconnect safety: the same client command id replays its original receipt
/// rather than executing twice.
#[test]
fn a_duplicate_client_command_id_replays_the_original_receipt() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);
    let envelope = pass(&request, "cmd-1");

    let mut ledger = CommandLedger::new();
    assert!(admit(&envelope, &context, &ledger).is_none());

    let receipt = CommandReceipt {
        match_id: match_id(),
        seat: SEAT_A,
        request: request.id,
        client_command_id: ClientCommandId::new("cmd-1"),
        kind: CommandKind::PassPriority,
        previous_revision: revision(5),
        revision: revision(6),
        controller: "scripted:test".to_owned(),
    };
    assert!(ledger.record(receipt.clone()).is_none());
    assert_eq!(ledger.len(), 1);

    let replay = admit(&envelope, &context, &ledger).expect("must be a duplicate");
    assert_eq!(replay, CommandResult::Duplicate { receipt });
    assert!(replay.is_inert());
}

/// A retry after the state has already advanced is the realistic reconnect
/// case: it must be reported as a duplicate, not as a staleness error, or a
/// correct client looks like a protocol violator.
#[test]
fn a_duplicate_is_detected_before_staleness() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let envelope = pass(&request, "cmd-1");

    let mut ledger = CommandLedger::new();
    ledger.record(CommandReceipt {
        match_id: match_id(),
        seat: SEAT_A,
        request: request.id,
        client_command_id: ClientCommandId::new("cmd-1"),
        kind: CommandKind::PassPriority,
        previous_revision: revision(5),
        revision: revision(6),
        controller: "scripted:test".to_owned(),
    });

    // The match has moved on since the original submission.
    let later = priority_request(SEAT_B, revision(6), 13);
    let later_context = SubmissionContext::from_request(&later, &SEATS);

    let result =
        admit(&envelope, &later_context, &ledger).expect("must be resolved without the engine");
    assert!(
        matches!(result, CommandResult::Duplicate { .. }),
        "a reconnect retry must replay, not report staleness: {result:?}"
    );
}

/// An unrelated client command id is not a duplicate, even from the same seat
/// in the same request.
#[test]
fn distinct_client_command_ids_are_independent() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);

    let mut ledger = CommandLedger::new();
    assert!(ledger.is_empty());
    ledger.record(CommandReceipt {
        match_id: match_id(),
        seat: SEAT_A,
        request: request.id,
        client_command_id: ClientCommandId::new("cmd-1"),
        kind: CommandKind::PassPriority,
        previous_revision: revision(5),
        revision: revision(6),
        controller: "scripted:test".to_owned(),
    });

    assert!(admit(&pass(&request, "cmd-2"), &context, &ledger).is_none());
}

/// Validation order is itself a contract: an envelope that is wrong in several
/// ways reports the most fundamental problem, so a client fixes the right one
/// first.
#[test]
fn the_most_fundamental_violation_is_reported_first() {
    let request = priority_request(SEAT_A, revision(5), 12);
    let context = SubmissionContext::from_request(&request, &SEATS);

    let mut everything_wrong = pass(&request, "cmd-1");
    everything_wrong.schema = SchemaVersion::new(2, 0, 0);
    everything_wrong.match_id = MatchId::new("other");
    everything_wrong.seat = SEAT_B;
    everything_wrong.request = ActionRequestId(999);
    everything_wrong.observed_revision = revision(1);
    assert!(matches!(
        validate_envelope(&everything_wrong, &context),
        Err(ProtocolError::SchemaMismatch { .. })
    ));

    let mut wrong_match_and_seat = pass(&request, "cmd-1");
    wrong_match_and_seat.match_id = MatchId::new("other");
    wrong_match_and_seat.seat = SEAT_B;
    assert!(matches!(
        validate_envelope(&wrong_match_and_seat, &context),
        Err(ProtocolError::MatchMismatch { .. })
    ));

    let mut wrong_seat_and_request = pass(&request, "cmd-1");
    wrong_seat_and_request.seat = SEAT_B;
    wrong_seat_and_request.request = ActionRequestId(999);
    assert!(matches!(
        validate_envelope(&wrong_seat_and_request, &context),
        Err(ProtocolError::WrongSeat { .. })
    ));

    let mut wrong_request_and_revision = pass(&request, "cmd-1");
    wrong_request_and_revision.request = ActionRequestId(999);
    wrong_request_and_revision.observed_revision = revision(1);
    assert!(matches!(
        validate_envelope(&wrong_request_and_revision, &context),
        Err(ProtocolError::StaleRequest { .. })
    ));
}

/// `ActionRequest::reply` is the only assembly path a controller should use,
/// and it must produce an envelope that validates by construction.
#[test]
fn replies_built_from_a_request_always_validate() {
    for (seat, sequence, id) in [(SEAT_A, 1, 1), (SEAT_B, 2, 2), (SEAT_A, 3, 3)] {
        let request = priority_request(seat, revision(sequence), id);
        let context = SubmissionContext::from_request(&request, &SEATS);
        let envelope = request.reply(ClientCommandId::new("c"), GameCommand::PassPriority);
        assert_eq!(validate_envelope(&envelope, &context), Ok(()));
    }
}

fn prompt(decision: DecisionRef) -> cardbench_magic_protocol::DecisionPrompt {
    cardbench_magic_protocol::DecisionPrompt {
        decision,
        kind: cardbench_magic_protocol::DecisionKindLabel::new("Draw"),
        visibility: cardbench_magic_protocol::DecisionVisibilityDto::Private,
        min_selections: 1,
        max_selections: 1,
        target_candidates: Vec::new(),
        candidates: Vec::new(),
        trigger_candidates: Vec::new(),
        replacement_candidates: Vec::new(),
        color_candidates: Vec::new(),
        card_name_candidates: Vec::new(),
    }
}
