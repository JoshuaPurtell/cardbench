//! Requests for a seat to act, and the legal-action surface attached to them.
//!
//! A request is the only thing a controller is given. It carries the viewer's
//! observation, the identity a reply must echo, and whatever description of
//! legal commands the engine can currently derive. A controller that needs
//! anything else is reaching around the boundary.

use crate::command::{CommandEnvelope, GameCommand};
use crate::ids::{
    ActionRequestId, CardName, ClientCommandId, DecisionRef, MatchId, ObjectRef, SeatId,
    StateRevision,
};
use crate::observation::MatchObservation;
use crate::primitives::{ColorDto, ReplacementChoiceDto, TargetDto, TriggerOrderEntryDto};
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

/// Why a seat is being asked to act.
///
/// This distinguishes an ordinary priority window from a turn-based action and
/// from a mandatory rules decision, because the three have different
/// stale-safety rules: a priority window is identified by the revision alone,
/// while a decision additionally carries the engine's own decision identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ActionRequestKind {
    /// The seat holds priority and may cast, activate, play a land, or pass.
    Priority,
    /// The active seat must declare attackers, possibly none.
    DeclareAttackers,
    /// A defending seat must declare blockers, possibly none.
    DeclareBlockers,
    /// The seat must take its draw, or replace it.
    Draw,
    /// The seat may decline or accept an optional triggered ability.
    OptionalTriggeredAbility,
    /// The seat must choose from its own library, which no other seat sees.
    PrivateLibraryChoice,
    /// The seat must choose from an opponent's library, which that opponent
    /// does not see.
    PrivateOpponentLibraryChoice,
    /// The seat must resolve a library search.
    LibrarySearchChoice,
    /// The seat must answer a typed pending decision.
    PendingDecision,
}

impl ActionRequestKind {
    /// Whether a command answering this request must echo a decision identity.
    #[must_use]
    pub const fn requires_decision(self) -> bool {
        !matches!(
            self,
            Self::Priority | Self::DeclareAttackers | Self::DeclareBlockers
        )
    }
}

/// Whether a decision's content may be shown outside the deciding seat.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DecisionVisibilityDto {
    /// The prompt and its options are public information.
    Public,
    /// Only the deciding seat may see the options. These must never enter the
    /// public event stream.
    Private,
}

/// The engine's decision-kind discriminant, as an owned label.
///
/// This is a label rather than a mirrored enum on purpose. The rules engine's
/// decision taxonomy grows with every card mechanic; freezing a copy of it
/// into a versioned wire schema would force a protocol bump for work that does
/// not change the transport contract at all. The *structure* a client needs to
/// answer a decision is carried by the typed candidate lists below, which are
/// mirrored exactly; the label is for display, metrics, and policy dispatch.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct DecisionKindLabel(pub String);

impl DecisionKindLabel {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DecisionKindLabel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A pending rules decision, projected for the deciding seat.
///
/// The candidate lists mirror the engine's pending-decision view field for
/// field. Empty lists mean "this decision does not offer that kind of
/// candidate", not "no candidates are available".
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecisionPrompt {
    pub decision: DecisionRef,
    pub kind: DecisionKindLabel,
    pub visibility: DecisionVisibilityDto,
    pub min_selections: u8,
    pub max_selections: u8,
    pub target_candidates: Vec<TargetDto>,
    pub candidates: Vec<ObjectRef>,
    pub trigger_candidates: Vec<TriggerOrderEntryDto>,
    pub replacement_candidates: Vec<ReplacementChoiceDto>,
    pub color_candidates: Vec<ColorDto>,
    pub card_name_candidates: Vec<CardName>,
}

impl DecisionPrompt {
    /// Whether this prompt discloses anything a non-deciding viewer must not
    /// learn.
    ///
    /// A private prompt is not merely flagged: the projection layer must omit
    /// it entirely from other viewers' requests and from the public event
    /// stream.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        matches!(self.visibility, DecisionVisibilityDto::Private)
    }
}

/// One command the engine currently reports as legal, with the identity a
/// submission must echo.
///
/// Carrying the request and revision on every option is what makes the option
/// self-validating: a controller that caches options across a state change
/// submits a provably stale envelope rather than a plausible-looking one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LegalActionOption {
    pub request: ActionRequestId,
    pub revision: StateRevision,
    pub command: GameCommand,
    /// Short human-facing description, for UI and debugging only. Never
    /// parsed.
    pub summary: String,
}

/// What the engine can say about the legal commands for one request.
///
/// `exhaustive` is the honest part. Enumerating every legal command in Magic
/// means enumerating target and payment combinatorics, which this build does
/// not do. Until it does, this surface is a *subset* and a controller must not
/// treat an absent option as illegal.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LegalActionSurface {
    /// True only when every legal command is present. Reported by
    /// [`crate::FeatureCapability::ExhaustiveLegalActions`].
    pub exhaustive: bool,
    pub options: Vec<LegalActionOption>,
    /// Command kinds the engine knows are available but did not enumerate,
    /// because doing so would require expanding a combinatorial space.
    pub unenumerated_kinds: Vec<crate::command::CommandKind>,
}

impl LegalActionSurface {
    /// A surface that enumerates nothing and claims nothing.
    #[must_use]
    pub fn unavailable() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    /// Whether `command` appears verbatim in the enumerated subset.
    ///
    /// A `false` result is only meaningful when [`Self::exhaustive`] is true.
    #[must_use]
    pub fn contains(&self, command: &GameCommand) -> bool {
        self.options.iter().any(|option| &option.command == command)
    }
}

/// The engine asking one seat to act.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionRequest {
    pub schema: SchemaVersion,
    pub match_id: MatchId,
    pub id: ActionRequestId,
    /// The seat that must reply. No other seat's command may satisfy this
    /// request.
    pub seat: SeatId,
    /// The revision this request was issued at. A reply must echo it exactly.
    pub revision: StateRevision,
    pub kind: ActionRequestKind,
    /// Present exactly when [`ActionRequestKind::requires_decision`] holds.
    pub decision: Option<DecisionPrompt>,
    /// The acting seat's view of the match at `revision`.
    pub observation: MatchObservation,
    pub legal: LegalActionSurface,
}

impl ActionRequest {
    /// The decision identity a reply must echo, if any.
    #[must_use]
    pub fn expected_decision(&self) -> Option<DecisionRef> {
        self.decision.as_ref().map(|prompt| prompt.decision)
    }

    /// Builds a correctly attributed envelope for `command`.
    ///
    /// Every controller should reply through this rather than assembling an
    /// envelope by hand, so that request, revision, seat, and match identity
    /// cannot drift from the request being answered.
    #[must_use]
    pub fn reply(
        &self,
        client_command_id: ClientCommandId,
        command: GameCommand,
    ) -> CommandEnvelope {
        CommandEnvelope {
            schema: self.schema,
            match_id: self.match_id.clone(),
            seat: self.seat,
            observed_revision: self.revision,
            request: self.id,
            client_command_id,
            command,
        }
    }

    /// Whether this request's shape is internally consistent.
    ///
    /// A request that claims to need a decision but carries no prompt, or
    /// carries a prompt it does not need, is a projection bug. Checking it
    /// here means the contract tests catch it at the boundary rather than a
    /// controller failing mysteriously later.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.kind.requires_decision() == self.decision.is_some()
            && self
                .legal
                .options
                .iter()
                .all(|option| option.request == self.id && option.revision == self.revision)
    }
}
