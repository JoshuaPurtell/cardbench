//! How a match ended.

use crate::ids::{SeatId, StateRevision, TeamId};
use serde::{Deserialize, Serialize};

/// A completed match's outcome.
///
/// Winners are a set of seats *and* a set of teams rather than one optional
/// seat. A duel is the degenerate case; a team format or a multi-seat draw
/// needs no new shape and no client change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalResult {
    pub revision: StateRevision,
    pub outcome: TerminalOutcome,
    /// Seats that won, in seat order. Empty for a draw.
    pub winning_seats: Vec<SeatId>,
    /// Teams that won, in team order. Empty for a draw.
    pub winning_teams: Vec<TeamId>,
    /// Every seat that lost, in the order it was eliminated. A seat that was
    /// still alive at a simultaneous-loss draw appears here too.
    pub eliminated_seats: Vec<SeatId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TerminalOutcome {
    /// Exactly one seat or team remains.
    Win,
    /// No seat or team remains, for example a simultaneous loss.
    Draw,
    /// The match was stopped by an external bound (move or turn limit) rather
    /// than by the rules. This is not a rules outcome and must never be
    /// reported as a win.
    Truncated,
}

impl TerminalResult {
    #[must_use]
    pub fn draw(revision: StateRevision, eliminated_seats: Vec<SeatId>) -> Self {
        Self {
            revision,
            outcome: TerminalOutcome::Draw,
            winning_seats: Vec::new(),
            winning_teams: Vec::new(),
            eliminated_seats,
        }
    }

    #[must_use]
    pub const fn is_rules_outcome(&self) -> bool {
        matches!(self.outcome, TerminalOutcome::Win | TerminalOutcome::Draw)
    }
}
