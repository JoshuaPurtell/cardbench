//! Versioned archetype policies.
//!
//! Every version is kept, never edited in place. That is the whole measurement
//! apparatus: "v2 is stronger than v1" is only meaningful if v1 is still
//! runnable, and a policy improved in place destroys the baseline it would be
//! compared against.
//!
//! Each version pilots *any* deck, driven by [`crate::Archetype`] weights, so a
//! version's strength is measured across a range of decks rather than on the
//! one deck it was tuned against. A change that helps Boros aggro and hurts
//! Golgari midrange is not an improvement, and only a multi-deck ladder can
//! see that.

use crate::CodePolicy;
use crate::archetype::Archetype;
use crate::planner::CardIndex;
use cardbench_magic_engine::PlayerId;
use std::sync::Arc;

pub mod v1;
pub mod v2;
pub mod v3;
pub mod v4;
pub mod v5;
pub mod v6;
pub mod v7;
pub mod v8;

/// One generation of the archetype policy.
///
/// Ordered oldest to newest so a ladder can walk successive pairs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PolicyVersion {
    /// Shared planners: mana payment, one-ply combat, target scoring.
    V1,
    /// Adds whole-set attack planning instead of judging each attacker against
    /// a hypothetically free blocker.
    V2,
    /// Adds valued blocking: a block is worth the damage it stops, not only
    /// the material it trades.
    V3,
    /// Adds land sequencing driven by what a land makes castable this turn.
    V4,
    /// Adds activated abilities: pingers, token engines, and tappers.
    V5,
    /// Adds instant timing: hold interaction for the window where it is worth
    /// the most instead of casting it in your own upkeep.
    V6,
    /// Splits instant timing by what the spell does: removal answers the board
    /// on my turn, reach is still held for the end step or for lethal.
    V7,
    /// Plans attacks against the valued defender used by the real blocker.
    V8,
}

impl PolicyVersion {
    pub const ALL: [Self; 8] = [
        Self::V1,
        Self::V2,
        Self::V3,
        Self::V4,
        Self::V5,
        Self::V6,
        Self::V7,
        Self::V8,
    ];

    /// The newest version. Campaigns that do not care about history use this.
    #[must_use]
    pub const fn latest() -> Self {
        Self::V8
    }

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
            Self::V3 => "v3",
            Self::V4 => "v4",
            Self::V5 => "v5",
            Self::V6 => "v6",
            Self::V7 => "v7",
            Self::V8 => "v8",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|version| version.id() == value)
    }

    /// The version immediately before this one, for ladder comparisons.
    #[must_use]
    pub const fn previous(self) -> Option<Self> {
        match self {
            Self::V1 => None,
            Self::V2 => Some(Self::V1),
            Self::V3 => Some(Self::V2),
            Self::V4 => Some(Self::V3),
            Self::V5 => Some(Self::V4),
            Self::V6 => Some(Self::V5),
            Self::V7 => Some(Self::V6),
            Self::V8 => Some(Self::V7),
        }
    }
}

impl std::fmt::Display for PolicyVersion {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Builds the pilot for one seat at one version.
#[must_use]
pub fn seat_policy(
    version: PolicyVersion,
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
) -> Box<dyn CodePolicy> {
    match version {
        PolicyVersion::V1 => Box::new(v1::ArchetypePolicyV1::new(player, archetype, index)),
        PolicyVersion::V2 => Box::new(v2::ArchetypePolicyV2::new(player, archetype, index)),
        PolicyVersion::V3 => Box::new(v3::ArchetypePolicyV3::new(player, archetype, index)),
        PolicyVersion::V4 => Box::new(v4::ArchetypePolicyV4::new(player, archetype, index)),
        PolicyVersion::V5 => Box::new(v5::ArchetypePolicyV5::new(player, archetype, index)),
        PolicyVersion::V6 => Box::new(v6::ArchetypePolicyV6::new(player, archetype, index)),
        PolicyVersion::V7 => Box::new(v7::ArchetypePolicyV7::new(player, archetype, index)),
        PolicyVersion::V8 => Box::new(v8::new(player, archetype, index)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_version_round_trips_and_chains() {
        for version in PolicyVersion::ALL {
            assert_eq!(PolicyVersion::parse(version.id()), Some(version));
        }
        assert_eq!(PolicyVersion::V1.previous(), None);
        assert_eq!(PolicyVersion::V3.previous(), Some(PolicyVersion::V2));
        assert_eq!(PolicyVersion::latest(), PolicyVersion::V8);
        assert_eq!(PolicyVersion::V8.previous(), Some(PolicyVersion::V7));
        assert_eq!(PolicyVersion::V7.previous(), Some(PolicyVersion::V6));
        assert_eq!(PolicyVersion::V6.previous(), Some(PolicyVersion::V5));
        assert_eq!(PolicyVersion::V4.previous(), Some(PolicyVersion::V3));
    }

    /// Each version must present a distinct policy id, or campaign receipts
    /// cannot attribute a move to the generation that made it.
    #[test]
    fn every_version_has_a_distinct_policy_id_per_archetype() {
        let index = crate::deck_match::shared_card_index();
        let mut seen = std::collections::BTreeSet::new();
        for version in PolicyVersion::ALL {
            for archetype in Archetype::ALL {
                let policy = seat_policy(version, PlayerId(0), archetype, index.clone());
                assert!(
                    seen.insert(policy.id()),
                    "{version} {archetype} reuses policy id {}",
                    policy.id()
                );
            }
        }
    }
}
