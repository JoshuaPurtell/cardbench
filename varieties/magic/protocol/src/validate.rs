//! Pure submission validation and the idempotency ledger.
//!
//! Nothing here touches a rules engine. That is the point: the checks that
//! guarantee "no stale command mutates a later state" are ordinary total
//! functions over owned data, so they can be exhaustively tested without
//! constructing a game, and an orchestrator cannot accidentally skip one by
//! taking a different code path into the engine.

use crate::command::{CommandEnvelope, CommandReceipt, CommandResult, ProtocolError};
use crate::ids::{ActionRequestId, ClientCommandId, DecisionRef, MatchId, SeatId, StateRevision};
use crate::request::ActionRequest;
use crate::version::{PROTOCOL_SCHEMA_VERSION, SchemaVersion};
use std::collections::BTreeMap;

/// The authoritative state one envelope is validated against.
///
/// Borrowed rather than owned so an orchestrator can build it from its live
/// request without cloning an observation on every submission.
#[derive(Clone, Copy, Debug)]
pub struct SubmissionContext<'a> {
    pub match_id: &'a MatchId,
    /// The single request currently open. There is never more than one.
    pub request: ActionRequestId,
    /// The seat that request was issued to.
    pub seat: SeatId,
    /// The revision that request was issued at.
    pub revision: StateRevision,
    /// The rules-level decision the request wraps, if any.
    pub decision: Option<DecisionRef>,
    /// Every seat that exists in this match, including eliminated ones.
    pub known_seats: &'a [SeatId],
}

impl<'a> SubmissionContext<'a> {
    /// Derives the context from the request the orchestrator most recently
    /// issued.
    ///
    /// Using this rather than assembling a context by hand keeps the validated
    /// identity and the issued identity from drifting apart.
    #[must_use]
    pub fn from_request(request: &'a ActionRequest, known_seats: &'a [SeatId]) -> Self {
        Self {
            match_id: &request.match_id,
            request: request.id,
            seat: request.seat,
            revision: request.revision,
            decision: request.expected_decision(),
            known_seats,
        }
    }
}

/// Validates one envelope against the open request.
///
/// The order is fixed and load-bearing. Schema comes first because a peer that
/// does not share the vocabulary cannot be told anything more specific;
/// identity comes before freshness because reporting a stale revision to a
/// client that is looking at a different match would be actively misleading.
///
/// # Errors
///
/// Returns the first violated precondition. A caller must treat any error as
/// "nothing happened": no state may be mutated on this path.
pub fn validate_envelope(
    envelope: &CommandEnvelope,
    context: &SubmissionContext<'_>,
) -> Result<(), ProtocolError> {
    validate_schema(envelope.schema)?;

    if &envelope.match_id != context.match_id {
        return Err(ProtocolError::MatchMismatch {
            expected: context.match_id.clone(),
            actual: envelope.match_id.clone(),
        });
    }

    if !context.known_seats.contains(&envelope.seat) {
        return Err(ProtocolError::UnknownSeat {
            seat: envelope.seat,
        });
    }

    if envelope.seat != context.seat {
        return Err(ProtocolError::WrongSeat {
            expected: context.seat,
            actual: envelope.seat,
        });
    }

    if envelope.request != context.request {
        return Err(ProtocolError::StaleRequest {
            observed: envelope.request,
            current: context.request,
        });
    }

    if envelope.observed_revision != context.revision {
        return Err(ProtocolError::StaleRevision {
            observed: envelope.observed_revision,
            current: context.revision,
        });
    }

    let echoed = envelope.command.decision();
    if echoed != context.decision {
        return Err(ProtocolError::StaleDecision {
            observed: echoed,
            expected: context.decision,
        });
    }

    Ok(())
}

/// Checks one schema version against this build's.
///
/// # Errors
///
/// Returns [`ProtocolError::SchemaMismatch`] when the major versions differ.
pub fn validate_schema(schema: SchemaVersion) -> Result<(), ProtocolError> {
    if PROTOCOL_SCHEMA_VERSION.is_compatible_with(schema) {
        Ok(())
    } else {
        Err(ProtocolError::SchemaMismatch {
            expected: PROTOCOL_SCHEMA_VERSION,
            actual: schema,
        })
    }
}

/// Per-match record of which client command ids have already been applied.
///
/// A reconnecting client that is unsure whether its last submission landed
/// resubmits the same id and receives the original receipt. Without this,
/// "did my attack go through?" is unanswerable over any transport that can
/// drop a response.
#[derive(Clone, Debug, Default)]
pub struct CommandLedger {
    applied: BTreeMap<ClientCommandId, CommandReceipt>,
}

impl CommandLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The receipt previously issued for `id`, if any.
    #[must_use]
    pub fn lookup(&self, id: &ClientCommandId) -> Option<&CommandReceipt> {
        self.applied.get(id)
    }

    /// Records an accepted command's receipt.
    ///
    /// Returns the previously stored receipt when the id was already present,
    /// which is always a caller bug: the duplicate should have been caught by
    /// [`Self::check`] before the engine ran.
    pub fn record(&mut self, receipt: CommandReceipt) -> Option<CommandReceipt> {
        self.applied
            .insert(receipt.client_command_id.clone(), receipt)
    }

    /// The replayed outcome for an already-applied id.
    ///
    /// Deliberately checked *before* freshness validation: a duplicate of an
    /// accepted command necessarily observes a now-stale revision, and
    /// reporting that as a staleness error would make a correct client's
    /// retry look like a protocol violation.
    #[must_use]
    pub fn check(&self, id: &ClientCommandId) -> Option<CommandResult> {
        self.lookup(id).map(|receipt| CommandResult::Duplicate {
            receipt: receipt.clone(),
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.applied.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty()
    }
}

/// Full pre-engine admission: idempotency first, then validation.
///
/// `None` means the caller should proceed to the rules engine. `Some` is a
/// finished [`CommandResult`] that must be returned to the client without the
/// engine being touched at all — which is how "rejected commands produce no
/// partial mutation" is enforced structurally rather than by discipline.
#[must_use]
pub fn admit(
    envelope: &CommandEnvelope,
    context: &SubmissionContext<'_>,
    ledger: &CommandLedger,
) -> Option<CommandResult> {
    if let Some(duplicate) = ledger.check(&envelope.client_command_id) {
        return Some(duplicate);
    }
    validate_envelope(envelope, context)
        .err()
        .map(|error| CommandResult::Rejected {
            error,
            revision: context.revision,
        })
}
