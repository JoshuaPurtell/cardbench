//! Generation 8 of the archetype policy.
//!
//! v8 plans attacks against the valued defender that v3 introduced for real
//! blocking. The timing, mana, targeting, and decision code remains shared
//! with v7; only the attack-planning mode changes.

use super::v7::ArchetypePolicyV7;
use crate::archetype::Archetype;
use crate::planner::CardIndex;
use cardbench_magic_engine::PlayerId;
use std::sync::Arc;

/// The v8 public policy type. Its constructor selects the new attack model.
pub type ArchetypePolicyV8 = ArchetypePolicyV7;

/// Builds one v8 seat.
#[must_use]
pub fn new(player: PlayerId, archetype: Archetype, index: Arc<CardIndex>) -> ArchetypePolicyV8 {
    ArchetypePolicyV7::new_valued(player, archetype, index)
}
