//! `cardbench/magic/code_policy` — the deck-and-pilot evaluation surface.
//!
//! Three pieces, in the order they are load-bearing:
//!
//! * [`reference`] — `reference_v1`, a named freeze of `archetypes::v5`. The
//!   ranking origin. Every score is a delta against it, so it must not move.
//! * [`roster`] — the fixed surface: five visible train opponents, five sealed
//!   held-out opponents, and the cell coordinates both arms play.
//! * [`sweep`] — the scorer. Paired seats, Wilson intervals, per-opponent
//!   reporting, fail-closed coverage, and a delta reward.
//!
//! # Why the reward is a delta
//!
//! An absolute win rate is not comparable to itself across runs. It moves when
//! the opponent split changes, when the seed count changes, and when the engine
//! changes — none of which is the candidate. Scoring against a frozen reference
//! that played the identical cells removes all three at once, and is the rule
//! `docs/CODE_POLICY_DEO_DESIGN.md` §3.3 sets.
//!
//! # Why the held-out split exists
//!
//! Without it, "run more seeds" is training on the test set and hardcoding is a
//! winning strategy. The train five exist so a candidate can iterate without a
//! blind budget; the held-out five are what the reward is computed from, and the
//! gap between the two numbers is the overfitting the benchmark is there to
//! expose.

pub mod reference;
pub mod roster;
pub mod sha256;
pub mod sweep;
