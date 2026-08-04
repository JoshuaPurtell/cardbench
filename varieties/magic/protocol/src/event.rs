//! The viewer-redacted event stream.
//!
//! The rules engine already keeps hidden candidates out of its canonical log:
//! a decision receipt names the decision, its owner, and its kind, never the
//! options that were offered. This layer preserves that property across the
//! transport and adds the machinery that non-rules events (draft picks, for
//! example) will need, without pretending those exist yet.

use crate::ids::{SeatId, StateRevision, TeamId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Who may receive one event record.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum EventVisibility {
    /// Everyone, including spectators.
    Public,
    /// Only the listed seats, and any authority scope.
    Seats(BTreeSet<SeatId>),
    /// Only seats on the named team, and any authority scope.
    Team(TeamId),
    /// Authority scopes only. Never delivered to a playing seat.
    JudgeOnly,
}

/// One record in a match's authoritative event stream.
///
/// `canonical` is the exact engine replay line and is what the digest is
/// computed over. It is present only where `visibility` permits: a redacted
/// record keeps its sequence and kind so a client's cursor stays aligned,
/// but carries no payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventRecord {
    /// Zero-based position in the authoritative stream. Stable across
    /// reconnects and used as the replay cursor.
    pub sequence: u64,
    /// The revision this event was sealed at.
    pub revision: StateRevision,
    pub visibility: EventVisibility,
    /// The engine event variant name. Always disclosed: knowing that *a*
    /// decision completed is public information; knowing its content is not.
    pub kind: String,
    /// Seats this event names, for client routing.
    pub seats: Vec<SeatId>,
    /// The canonical replay payload, when entitled.
    pub canonical: Option<String>,
}

impl EventRecord {
    /// A fully public record.
    #[must_use]
    pub fn public(
        sequence: u64,
        revision: StateRevision,
        kind: impl Into<String>,
        seats: Vec<SeatId>,
        canonical: impl Into<String>,
    ) -> Self {
        Self {
            sequence,
            revision,
            visibility: EventVisibility::Public,
            kind: kind.into(),
            seats,
            canonical: Some(canonical.into()),
        }
    }

    /// Whether `scope` may receive this record's payload.
    #[must_use]
    pub fn is_visible_to(
        &self,
        scope: crate::ViewerScope,
        membership: &crate::ScopeMembership,
    ) -> bool {
        if scope.is_authoritative() {
            return true;
        }
        match &self.visibility {
            EventVisibility::Public => true,
            EventVisibility::Seats(seats) => seats
                .iter()
                .any(|seat| scope.may_see_private(*seat, membership)),
            EventVisibility::Team(team) => {
                matches!(scope, crate::ViewerScope::Team(viewer) if viewer == *team)
            }
            EventVisibility::JudgeOnly => false,
        }
    }

    /// This record as `scope` should receive it.
    ///
    /// Never returns `None`: dropping the record outright would desynchronise
    /// the receiver's cursor. An unentitled viewer gets the position and kind
    /// with no payload.
    #[must_use]
    pub fn redacted_for(
        &self,
        scope: crate::ViewerScope,
        membership: &crate::ScopeMembership,
    ) -> Self {
        if self.is_visible_to(scope, membership) {
            return self.clone();
        }
        Self {
            sequence: self.sequence,
            revision: self.revision,
            visibility: self.visibility.clone(),
            kind: self.kind.clone(),
            seats: Vec::new(),
            canonical: None,
        }
    }
}

/// A contiguous slice of the event stream, as delivered to one viewer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventStreamPage {
    /// Sequence of the first record the caller had already seen, or 0.
    pub from_sequence: u64,
    pub records: Vec<EventRecord>,
    /// The revision the stream is current as of.
    pub revision: StateRevision,
}
