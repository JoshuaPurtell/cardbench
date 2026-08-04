//! Schema identity and the honest capability manifest.
//!
//! Every observation, request, and command carries [`SchemaVersion`]. A peer
//! that cannot understand the major version must reject rather than guess.
//!
//! [`ProtocolCapabilities`] exists so a client never has to infer support from
//! the absence of an error. Formats and features that are modelled by the
//! transport schema but not yet implemented by the rules engine are reported
//! as [`SupportLevel::Modelled`], never as supported.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

/// Stable protocol family name. Transports should reject a manifest whose
/// name differs rather than attempting a structural match.
pub const PROTOCOL_NAME: &str = "cardbench.magic.protocol";

/// The schema version produced by this build.
pub const PROTOCOL_SCHEMA_VERSION: SchemaVersion = SchemaVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

/// A semantic protocol schema version.
///
/// Major is the compatibility boundary: a differing major means the peers do
/// not share a command or observation vocabulary and must not interoperate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl SchemaVersion {
    #[must_use]
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Whether a peer advertising `self` can exchange messages with `other`.
    ///
    /// This is deliberately major-only. A minor addition must never change the
    /// meaning of an existing field, so an older peer stays compatible.
    #[must_use]
    pub const fn is_compatible_with(self, other: Self) -> bool {
        self.major == other.major
    }
}

impl Display for SchemaVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// How completely one named format or feature is actually implemented.
///
/// The distinction between [`Self::Modelled`] and [`Self::Supported`] is the
/// whole point of this enum: the protocol intentionally carries shapes, such
/// as a per-attacker defender or a team identity, ahead of the rules work.
/// Declaring those as supported would be a false correctness claim.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum SupportLevel {
    /// Neither the schema nor the engine represents this.
    Absent,
    /// The transport schema can express it, but no rules implementation or
    /// test fixture backs it. Commands relying on it must be rejected with
    /// [`crate::ProtocolError::Unsupported`].
    Modelled,
    /// Implemented by the rules engine and covered by deterministic tests.
    Supported,
}

/// A named format whose rules are separate from the base game.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FormatCapability {
    /// Two-seat duel. The only format with engine coverage today.
    Duel,
    /// Three or more seats, every seat its own team.
    FreeForAll,
    Commander,
    TwoHeadedGiant,
    BoosterDraft,
}

/// A discrete interface capability a client may depend on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FeatureCapability {
    /// The attacking player selects which seat or team each attacker attacks.
    ChosenAttackDefender,
    /// More than one blocker may be assigned to a single attacker.
    MultipleBlockers,
    /// Team identities with shared life and shared turn structure.
    Teams,
    /// [`crate::ActionRequest`] carries an exhaustive enumeration of legal
    /// commands rather than a partial hint surface.
    ExhaustiveLegalActions,
    /// A duplicate client command id returns the original result instead of
    /// re-executing.
    IdempotentResubmission,
    /// Viewer-scoped redaction of observations and events.
    ScopedRedaction,
    /// Deterministic canonical event log and replay digest.
    CanonicalReplay,
}

/// What one peer can actually do, reported rather than inferred.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolCapabilities {
    pub protocol: String,
    pub schema: SchemaVersion,
    /// The `CardBench` variety this manifest describes.
    pub variety: String,
    /// Seat counts with deterministic fixture coverage. A count outside this
    /// list is not claimed to work even if the engine would construct it.
    pub verified_seat_counts: Vec<u16>,
    pub formats: BTreeMap<FormatCapability, SupportLevel>,
    pub features: BTreeMap<FeatureCapability, SupportLevel>,
}

impl ProtocolCapabilities {
    /// The manifest for this build.
    ///
    /// Seat counts and support levels here are claims that the contract tests
    /// in this crate and in `cardbench-magic-session` are expected to defend.
    #[must_use]
    pub fn current() -> Self {
        Self {
            protocol: PROTOCOL_NAME.to_owned(),
            schema: PROTOCOL_SCHEMA_VERSION,
            variety: "magic".to_owned(),
            verified_seat_counts: vec![2],
            formats: [
                (FormatCapability::Duel, SupportLevel::Supported),
                (FormatCapability::FreeForAll, SupportLevel::Modelled),
                (FormatCapability::Commander, SupportLevel::Absent),
                (FormatCapability::TwoHeadedGiant, SupportLevel::Absent),
                (FormatCapability::BoosterDraft, SupportLevel::Absent),
            ]
            .into_iter()
            .collect(),
            features: [
                (
                    FeatureCapability::ChosenAttackDefender,
                    SupportLevel::Modelled,
                ),
                (FeatureCapability::MultipleBlockers, SupportLevel::Absent),
                (FeatureCapability::Teams, SupportLevel::Modelled),
                (
                    FeatureCapability::ExhaustiveLegalActions,
                    SupportLevel::Absent,
                ),
                (
                    FeatureCapability::IdempotentResubmission,
                    SupportLevel::Modelled,
                ),
                (FeatureCapability::ScopedRedaction, SupportLevel::Supported),
                (FeatureCapability::CanonicalReplay, SupportLevel::Supported),
            ]
            .into_iter()
            .collect(),
        }
    }

    #[must_use]
    pub fn format(&self, format: FormatCapability) -> SupportLevel {
        self.formats
            .get(&format)
            .copied()
            .unwrap_or(SupportLevel::Absent)
    }

    #[must_use]
    pub fn feature(&self, feature: FeatureCapability) -> SupportLevel {
        self.features
            .get(&feature)
            .copied()
            .unwrap_or(SupportLevel::Absent)
    }
}
