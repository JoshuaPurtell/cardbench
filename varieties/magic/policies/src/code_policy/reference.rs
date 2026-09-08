//! `reference_v1` — the frozen ranking origin for `cardbench/magic/code_policy`.
//!
//! # What this is
//!
//! A **named, versioned freeze** of one archetype generation. It is not a new
//! policy and it is not an editable baseline: it is an alias with a guard, so
//! that every later generation becomes a *candidate measured against a fixed
//! origin* rather than an edit to the origin itself.
//!
//! ```text
//! reference_v1 := archetypes::v5
//! ```
//!
//! # What v5 carries
//!
//! v5's own module documentation states it: v2's whole-set attack planning,
//! v3's valued blocking, v4's land sequencing, and — new in v5 — activated
//! abilities. Nine of the thirty-eight distinct cards across the constructed
//! decks carry a stack-using activated ability and no generation before v5 ever
//! activated one; six of the nine (mana- or tap-cost) are handled, and the three
//! whose cost is a sacrifice are reported unsupported rather than skipped in
//! silence.
//!
//! Layered as:
//!
//! ```text
//! v1  shared planners: mana payment, one-ply combat, target scoring
//! v2  + whole-set attack planning
//! v3  + valued blocking (a block is worth the damage it stops)
//! v4  + land sequencing driven by what a land makes castable this turn
//! v5  + activated abilities: pingers, token engines, tappers        <-- frozen here
//! ```
//!
//! # Why v5 and not the newest generation
//!
//! Because `PolicyVersion::latest()` moves. A reference that tracks the newest
//! generation is not a reference — every generation would silently redefine the
//! origin and no cross-run number would be comparable. v5 is the last rung whose
//! contribution is a *capability* (it plays cards the earlier rungs could not
//! play at all) rather than a timing refinement, which makes it the natural
//! floor: competent enough that beating it is real work, old enough that the
//! v6/v7/v8 rungs already in the tree are candidates rather than baseline.
//!
//! This is deliberately the design in `docs/CODE_POLICY_DEO_DESIGN.md` §2.3.
//!
//! # How the freeze is enforced
//!
//! Two guards, both tests in this module:
//!
//! * **Identity.** The reference resolves to `PolicyVersion::V5` and its per-
//!   archetype policy ids are the literal `rav.archetype-*.v5` strings. Anyone
//!   repointing the alias fails here.
//! * **Behaviour.** A fixed mirror match at a fixed seed must reproduce a
//!   recorded event digest. An edit to `v5.rs`, or to any shared planner v5
//!   routes through, changes the transcript and fails here — which is the whole
//!   point, because a baseline that drifts invalidates every delta ever measured
//!   against it.
//!
//! A behavioural guard is what makes this a freeze rather than a naming
//! convention: `archetypes/mod.rs` promises versions are never edited in place,
//! and this is the test that holds the promise to account for the one version
//! the benchmark's arithmetic depends on.

use crate::CodePolicy;
use crate::archetype::Archetype;
use crate::archetypes::{PolicyVersion, seat_policy};
use crate::planner::CardIndex;
use cardbench_magic_engine::PlayerId;
use std::sync::Arc;

/// Stable identifier for the frozen reference, used in rosters and receipts.
pub const REFERENCE_ID: &str = "reference_v1";

/// The generation `reference_v1` freezes.
///
/// Deliberately a constant and not `PolicyVersion::latest()`.
pub const REFERENCE_VERSION: PolicyVersion = PolicyVersion::V5;

/// The archetype policy ids the freeze is pinned to, in [`Archetype::ALL`] order.
///
/// Pinned as literals rather than derived from the enum so that a rename of a
/// v5 id — which would silently break any receipt that recorded the old one —
/// is a test failure and not a surprise.
pub const REFERENCE_POLICY_IDS: [&str; 4] = [
    "rav.archetype-aggro.v5",
    "rav.archetype-midrange.v5",
    "rav.archetype-burn.v5",
    "rav.archetype-control.v5",
];

/// Builds one `reference_v1` seat.
///
/// Same signature as [`seat_policy`] minus the version, because the version is
/// the thing being frozen.
#[must_use]
pub fn reference_seat(
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy> {
    seat_policy(REFERENCE_VERSION, player, archetype, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck_match::{DeckMatchConfig, run_versioned_matchup, shared_card_index};

    /// The identity guard. Repointing the alias fails here.
    #[test]
    fn reference_v1_is_the_v5_generation() {
        assert_eq!(REFERENCE_VERSION, PolicyVersion::V5);
        assert_eq!(REFERENCE_VERSION.id(), "v5");
        let index = shared_card_index();
        for (archetype, expected) in Archetype::ALL.into_iter().zip(REFERENCE_POLICY_IDS) {
            let policy = reference_seat(PlayerId(0), archetype, index.clone());
            assert_eq!(
                policy.id(),
                expected,
                "reference_v1 must resolve to the frozen v5 policy for {archetype}"
            );
        }
    }

    /// The reference must not track the newest generation, or the origin moves
    /// under every measurement taken against it.
    #[test]
    fn reference_v1_does_not_track_latest() {
        assert_ne!(
            REFERENCE_VERSION,
            PolicyVersion::latest(),
            "a reference that follows `latest()` is not frozen; introduce reference_v2 \
             deliberately instead of letting a new generation redefine the origin"
        );
    }

    /// The behavioural guard. Any change to v5 or to a planner v5 routes
    /// through moves this digest.
    ///
    /// If this fails after an intentional change: that change altered the
    /// baseline. Publish a `reference_v2` and re-measure; do not update the
    /// constant to make the test green, because every previously recorded delta
    /// was computed against the old transcript.
    #[test]
    fn reference_v1_play_is_frozen_to_a_recorded_transcript() {
        let result = run_versioned_matchup(
            DeckMatchConfig {
                shuffle_seed: 73,
                ..DeckMatchConfig::default()
            },
            "rav_boros_aggro",
            "rav_boros_aggro",
            [REFERENCE_VERSION, REFERENCE_VERSION],
            Archetype::Aggro,
            shared_card_index(),
        )
        .expect("the frozen reference must be able to pilot a constructed deck");
        assert_eq!(
            result.digest, REFERENCE_FREEZE_DIGEST,
            "reference_v1 play changed; the frozen baseline drifted"
        );
    }

    /// Same frozen policy, rerun after the RAV printed-characteristic rules
    /// correction. Historical engine receipt: fnv1a64:5df7b930a827f3c3.
    /// This rebase is not a new policy-performance score.
    const REFERENCE_FREEZE_DIGEST: &str = "fnv1a64:9d9db75c31fad20a";
}
