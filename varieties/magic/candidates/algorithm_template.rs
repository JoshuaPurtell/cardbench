//! Starting point for a `cardbench/magic/code_policy` submission.
//!
//! Copy to `candidate/policy.rs` and edit. The grader compiles this file into
//! the sweep and calls `build_policy` once per seat, so the only hard
//! requirement is that the symbol exists with this signature.
//!
//! The shape below wraps the reference generation and forwards every decision
//! to it, which scores a delta of zero. That is deliberate: it means the first
//! thing you run is a working submission, and every point you gain afterwards
//! is attributable to a decision you actually changed. Starting from an empty
//! `propose_move` instead means reimplementing mana payment, combat and
//! targeting before you can play a legal game at all.
//!
//! `CodePolicy` has one required method, `propose_move`. Everything else --
//! pending decisions, optional triggers, draw replacement, library choices --
//! has a conservative default, and `inner` is there so you can delegate the
//! ones you do not want to think about yet.

use cardbench_magic_engine::{GameView, PlayerId, PolicyAction};
use cardbench_magic_policies::planner::CardIndex;
use cardbench_magic_policies::{Archetype, CodePolicy, PolicyVersion, seat_policy};
use std::sync::Arc;

pub struct CandidatePolicy {
    inner: Box<dyn CodePolicy>,
    /// Per-seat state lives here. One policy instance plays one seat of one
    /// game, so anything you keep is scoped to that game.
    plies: u32,
}

impl CodePolicy for CandidatePolicy {
    fn id(&self) -> &'static str {
        // Appears in engine findings and stall diagnostics. Make it specific
        // enough to tell two of your own submissions apart in a report.
        "candidate.template.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        self.plies += 1;
        // Replace with your own decision. Return the delegated action for the
        // situations you have not handled yet rather than a default one --
        // an action that is legal but pointless is scored the same as a bad
        // one, and a policy that stalls is scored worse than either.
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
}

pub fn build_policy(
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy> {
    Box::new(CandidatePolicy {
        // `archetype` is resolved from the deck you are flying, so the
        // delegate is already weighted for it.
        inner: seat_policy(PolicyVersion::V5, player, archetype, index),
        plies: 0,
    })
}
