//! Viewer scopes and the entitlement rules derived from them.
//!
//! Redaction is decided here, once, from a scope. No projection site is
//! allowed to invent its own "is this visible" test, because that is exactly
//! how a hidden zone leaks through one forgotten branch.

use crate::ids::{SeatId, TeamId};
use serde::{Deserialize, Serialize};

/// Who is looking at a match.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ViewerScope {
    /// One seat playing the match. Sees its own hidden zones and no others.
    Seat(SeatId),
    /// A team, for formats where teammates share hidden information. Sees
    /// every member seat's entitled information.
    Team(TeamId),
    /// A public observer. Sees only what every player already knows.
    Spectator,
    /// Full authority, for debugging and rules adjudication.
    ///
    /// This is never a scope an ordinary policy or client may hold. It exists
    /// so that judge tooling does not have to reach around the protocol.
    Judge,
    /// Full authority after the match has ended, when configured.
    PostGameReplay,
}

/// The seat-level membership a scope resolves to.
///
/// Passing this alongside a scope lets team entitlement be evaluated without
/// the redaction code needing the whole match roster.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ScopeMembership {
    /// Seats whose private information this scope may see.
    pub entitled_seats: Vec<SeatId>,
}

impl ScopeMembership {
    #[must_use]
    pub fn for_seat(seat: SeatId) -> Self {
        Self {
            entitled_seats: vec![seat],
        }
    }

    #[must_use]
    pub fn for_seats(seats: impl IntoIterator<Item = SeatId>) -> Self {
        let mut entitled_seats: Vec<SeatId> = seats.into_iter().collect();
        entitled_seats.sort_unstable();
        entitled_seats.dedup();
        Self { entitled_seats }
    }

    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn contains(&self, seat: SeatId) -> bool {
        self.entitled_seats.contains(&seat)
    }
}

impl ViewerScope {
    /// Whether this scope bypasses redaction entirely.
    ///
    /// Kept as one predicate so that adding an authority scope later cannot
    /// silently miss a redaction site.
    #[must_use]
    pub const fn is_authoritative(self) -> bool {
        matches!(self, Self::Judge | Self::PostGameReplay)
    }

    /// Whether this scope may see `seat`'s private information, given the
    /// membership resolved for it.
    #[must_use]
    pub fn may_see_private(self, seat: SeatId, membership: &ScopeMembership) -> bool {
        match self {
            Self::Judge | Self::PostGameReplay => true,
            Self::Seat(viewer) => viewer == seat,
            Self::Team(_) => membership.contains(seat),
            Self::Spectator => false,
        }
    }

    /// The seat acting under this scope, when there is exactly one.
    ///
    /// A team, spectator, or judge scope has no single acting seat, so a
    /// command claiming to come from one must be rejected rather than
    /// attributed to an arbitrary member.
    #[must_use]
    pub const fn acting_seat(self) -> Option<SeatId> {
        match self {
            Self::Seat(seat) => Some(seat),
            _ => None,
        }
    }
}
