//! Owned transport identities.
//!
//! Every identity here is a newtype rather than a bare integer so a seat can
//! never be passed where an object is expected. None of them borrow from the
//! engine: an identity that outlives one process is a hard requirement for
//! reconnect, replay, and any eventual network transport.

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// A match's durable identity. Opaque to the rules engine.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MatchId(pub String);

impl MatchId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for MatchId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One seat at the table. Seats are stable for a match's lifetime and remain
/// addressable after elimination, so a departed seat can still be projected
/// to spectators and still owns its historical event provenance.
///
/// A seat is deliberately not a team: see [`TeamId`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SeatId(pub u16);

impl Display for SeatId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "seat{}", self.0)
    }
}

/// A team of one or more seats.
///
/// Every seat belongs to exactly one team. In a free-for-all each team holds
/// exactly one seat, which keeps team-shaped code paths exercised long before
/// any shared-life format exists.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TeamId(pub u16);

impl Display for TeamId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "team{}", self.0)
    }
}

/// A point in a match's transition history.
///
/// `sequence` counts sealed engine transitions and is what a client compares
/// for staleness. `integrity` is the engine's public-state digest at that
/// point; carrying both means a client that somehow observes the same
/// sequence with a different state can detect it instead of silently
/// submitting against a state that was rebuilt underneath it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StateRevision {
    pub sequence: u64,
    pub integrity: u64,
}

impl StateRevision {
    #[must_use]
    pub const fn new(sequence: u64, integrity: u64) -> Self {
        Self {
            sequence,
            integrity,
        }
    }

    /// Whether `self` names a strictly earlier transition than `later`.
    #[must_use]
    pub const fn precedes(self, later: Self) -> bool {
        self.sequence < later.sequence
    }
}

impl Display for StateRevision {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "r{}#{:016x}", self.sequence, self.integrity)
    }
}

/// The identity of one outstanding request for a seat to act.
///
/// A request id is monotonic per match and is never reused, so a command
/// answering an earlier request cannot satisfy a later one even when the
/// board state happens to look identical.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ActionRequestId(pub u64);

impl Display for ActionRequestId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "req{}", self.0)
    }
}

/// A client-chosen idempotency key for one submitted command.
///
/// Resubmitting the same key against the same match must return the original
/// outcome rather than executing a second time. This is what makes reconnect
/// safe over an unreliable transport.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClientCommandId(pub String);

impl ClientCommandId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ClientCommandId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Transport mirror of the engine's rules-level decision identity.
///
/// This is distinct from [`ActionRequestId`]. A request is the orchestrator
/// asking a seat to act; a decision is the rules engine's own stale-safe
/// identity for a specific mandatory choice. A request that wraps a decision
/// carries both, and both must match for a command to be accepted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DecisionRef(pub u64);

impl Display for DecisionRef {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "decision{}", self.0)
    }
}

/// Transport mirror of a physical card object's engine identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ObjectRef(pub u64);

impl Display for ObjectRef {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "object{}", self.0)
    }
}

/// Transport mirror of one stack item's engine identity. A stack item is not
/// a card: two activations of the same permanent coexist with distinct ids.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct StackObjectRef(pub u64);

impl Display for StackObjectRef {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "stack{}", self.0)
    }
}

/// An owned card definition name.
///
/// The engine uses `&'static str` because its catalog is compiled in. That is
/// an implementation-facing type and must not reach a transport schema, so it
/// is copied into an owned value at the projection boundary.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CardName(pub String);

impl CardName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for CardName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// An owned ability identifier, for the same reason as [`CardName`].
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AbilityName(pub String);

impl AbilityName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for AbilityName {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
