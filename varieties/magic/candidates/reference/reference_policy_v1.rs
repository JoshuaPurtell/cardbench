//! The ranking origin, submitted through the candidate seam.
//!
//! This file grades to a delta of exactly `0.0` with an interval of `[0, 0]`,
//! because it builds the same seat the reference arm builds and both arms play
//! the identical cells. That is the control: a non-zero result here means the
//! pairing is broken -- the two arms are not playing the same games -- and no
//! candidate's number from that run means anything either.
//!
//! It is also the smallest complete example of the contract.

use cardbench_magic_engine::PlayerId;
use cardbench_magic_policies::code_policy::reference::reference_seat;
use cardbench_magic_policies::planner::CardIndex;
use cardbench_magic_policies::{Archetype, CodePolicy};
use std::sync::Arc;

/// The one symbol the grader looks for.
pub fn build_policy(
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy> {
    reference_seat(player, archetype, index)
}
