//! The command vocabulary and its submission envelope.
//!
//! One envelope carries exactly one command. There is no "candidate list"
//! shape here and there must never be one: a controller that submits several
//! guesses and lets the engine pick the first legal one hides its own
//! illegal proposals, which is precisely the audit property this variety has
//! and intends to keep.

use crate::ids::{
    AbilityName, ActionRequestId, ClientCommandId, DecisionRef, MatchId, ObjectRef, SeatId,
    StateRevision,
};
use crate::primitives::{
    ColorDto, DamageReplacementChoiceDto, ManaBundleDto, ReplacementChoiceDto, TargetDto,
    TriggerOrderEntryDto,
};
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

/// A seat's explicit mana allocation for one cost.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManaPaymentSelectionDto {
    pub generic: Vec<ColorDto>,
    pub hybrid: Vec<ColorDto>,
}

/// One mana ability activated while paying a cost. Mana abilities never use
/// the stack, so these are part of the enclosing command's atomic payment
/// rather than separate commands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PaymentManaAbilityDto {
    Bound(ManaAbilityActivationDto),
    BoundWithBundleChoice {
        activation: ManaAbilityActivationDto,
        chosen_bundle: ManaBundleDto,
    },
    BasicLand {
        land: ObjectRef,
        color: ColorDto,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ManaAbilityActivationDto {
    pub source: ObjectRef,
    pub ability: AbilityName,
    pub chosen_color: Option<ColorDto>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AbilityCostPaymentDto {
    pub counter_sources: Vec<ObjectRef>,
    pub return_permanents: Vec<ObjectRef>,
    pub hand_cards_to_library_top: Vec<ObjectRef>,
    pub graveyard_cards_to_exile: Vec<ObjectRef>,
    pub chosen_x: Option<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AbilityActivationDto {
    pub source: ObjectRef,
    pub ability: AbilityName,
    pub sacrifice_sources: Vec<ObjectRef>,
    pub additional_tap_creatures: Vec<ObjectRef>,
    pub discard_cards: Vec<ObjectRef>,
    pub targets: Vec<TargetDto>,
}

/// One creature tapped to help pay a Convoke cost.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConvokePaymentDto {
    pub creature: ObjectRef,
    pub contribution: ConvokeContributionDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConvokeContributionDto {
    Generic,
    Color(ColorDto),
}

/// An optional extra mana payment attached to one live battlefield source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExtraManaPaymentDto {
    pub source: ObjectRef,
    pub colors: Vec<ColorDto>,
}

/// The shared body of every cast command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CastRequestDto {
    pub card: ObjectRef,
    pub targets: Vec<TargetDto>,
    pub convoke: Vec<ConvokePaymentDto>,
    pub payment_mana_abilities: Vec<PaymentManaAbilityDto>,
}

/// The answer to a typed rules decision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DecisionSelectionDto {
    Objects(Vec<ObjectRef>),
    LibrarySearchAndCast {
        selected: Option<ObjectRef>,
        targets: Vec<TargetDto>,
    },
    ExiledSpellCopyCast {
        card: Option<ObjectRef>,
        targets: Vec<TargetDto>,
        mode: Option<u8>,
        color: Option<ColorDto>,
    },
    LibraryTopPartition {
        hand: ObjectRef,
        top: Option<ObjectRef>,
        /// Ordered bottom-to-top so restoring it discloses nothing.
        bottom: Vec<ObjectRef>,
    },
    TargetPlayerLibraryTopReorder {
        top: Vec<ObjectRef>,
        bottom: Vec<ObjectRef>,
    },
    Targets(Vec<TargetDto>),
    TriggerOrder(Vec<TriggerOrderEntryDto>),
    Replacements(Vec<ReplacementChoiceDto>),
    Color(ColorDto),
    CardName(crate::ids::CardName),
    CounterUnlessPaysMana {
        pay: bool,
        mana_abilities: Vec<PaymentManaAbilityDto>,
        mana_selection: ManaPaymentSelectionDto,
    },
    CounterUnlessDiscardsHand {
        discard: bool,
    },
}

/// Everything a seat can ask the rules engine to do.
///
/// This mirrors the engine's accepted action vocabulary one-for-one. Where
/// the engine uses `&'static str`, this uses an owned name; where it uses an
/// internal id, this uses a transport newtype. It adds no action the engine
/// cannot perform.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum GameCommand {
    PassPriority,
    Cast(CastRequestDto),
    CastWithMode {
        request: CastRequestDto,
        mode: u8,
    },
    CastWithModeAndColorChoice {
        request: CastRequestDto,
        mode: u8,
        color: ColorDto,
    },
    CastWithPayment {
        request: CastRequestDto,
        chosen_x: Option<u8>,
        mana_selection: ManaPaymentSelectionDto,
    },
    CastWithCreatureSpellAdditionalMana {
        request: CastRequestDto,
        chosen_x: Option<u8>,
        mana_selection: ManaPaymentSelectionDto,
        extra_payments: Vec<ExtraManaPaymentDto>,
    },
    CastWithColorChoice {
        request: CastRequestDto,
        color: ColorDto,
    },
    PlayLand {
        card: ObjectRef,
    },
    PlayLandWithEntryLifePayment {
        card: ObjectRef,
        pay_life: bool,
    },
    ActivateManaAbility {
        land: ObjectRef,
        color: ColorDto,
    },
    ActivateBoundManaAbility {
        activation: ManaAbilityActivationDto,
    },
    ActivateBoundManaAbilityWithBundleChoice {
        activation: ManaAbilityActivationDto,
        chosen_bundle: ManaBundleDto,
    },
    ActivateAbility {
        activation: AbilityActivationDto,
    },
    ActivateAbilityWithGeneralizedCosts {
        activation: AbilityActivationDto,
        cost_payment: AbilityCostPaymentDto,
        mana_payment_selection: Option<ManaPaymentSelectionDto>,
    },
    Transmute {
        card: ObjectRef,
    },
    DeclareAttackers {
        attackers: Vec<ObjectRef>,
    },
    DeclareBlockers {
        assignments: Vec<BlockAssignmentDto>,
    },
    /// Takes the normal draw, or replaces it with a legal dredge.
    Draw {
        decision: DecisionRef,
        dredge: Option<ObjectRef>,
    },
    ChoosePrivateLibraryCards {
        decision: DecisionRef,
        spell: ObjectRef,
        selected: Vec<ObjectRef>,
    },
    ChoosePrivateOpponentLibraryCardToExile {
        decision: DecisionRef,
        source: ObjectRef,
        ability: AbilityName,
        selected: Option<ObjectRef>,
    },
    ChooseLibrarySearchCard {
        decision: DecisionRef,
        source: ObjectRef,
        selected: Option<ObjectRef>,
    },
    ChooseTriggeredAbilityTargets {
        decision: DecisionRef,
        source: ObjectRef,
        ability: AbilityName,
        targets: Vec<TargetDto>,
    },
    ChooseTriggeredAbilityEffectObject {
        decision: DecisionRef,
        source: ObjectRef,
        ability: AbilityName,
        selected: Option<ObjectRef>,
    },
    ChooseDamageReplacement {
        decision: DecisionRef,
        source: ObjectRef,
        source_incarnation: u64,
        target: TargetDto,
        replacement: DamageReplacementChoiceDto,
    },
    ResolveOptionalTriggeredAbility {
        decision: DecisionRef,
        source: ObjectRef,
        ability: AbilityName,
        pay: bool,
        target: Option<TargetDto>,
    },
    SubmitDecision {
        decision: DecisionRef,
        selection: DecisionSelectionDto,
    },
    /// Reports that the engine cannot represent a rules situation. This is an
    /// audit signal, not a game action.
    ReportEngineWeakness {
        code: String,
        detail: String,
    },
}

/// One blocker assigned to one attacker.
///
/// The engine currently accepts at most one blocker per attacker; that limit
/// is reported through [`crate::FeatureCapability::MultipleBlockers`] rather
/// than encoded here, so lifting it needs no schema change.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BlockAssignmentDto {
    pub attacker: ObjectRef,
    pub blocker: ObjectRef,
}

impl GameCommand {
    /// The decision this command answers, if any.
    ///
    /// Used by envelope validation to reject a command that echoes a decision
    /// identity other than the one the request opened.
    #[must_use]
    pub const fn decision(&self) -> Option<DecisionRef> {
        match self {
            Self::Draw { decision, .. }
            | Self::ChoosePrivateLibraryCards { decision, .. }
            | Self::ChoosePrivateOpponentLibraryCardToExile { decision, .. }
            | Self::ChooseLibrarySearchCard { decision, .. }
            | Self::ChooseTriggeredAbilityTargets { decision, .. }
            | Self::ChooseTriggeredAbilityEffectObject { decision, .. }
            | Self::ChooseDamageReplacement { decision, .. }
            | Self::ResolveOptionalTriggeredAbility { decision, .. }
            | Self::SubmitDecision { decision, .. } => Some(*decision),
            _ => None,
        }
    }

    /// A stable, low-cardinality label for metrics and coverage reporting.
    #[must_use]
    pub const fn kind(&self) -> CommandKind {
        match self {
            Self::PassPriority => CommandKind::PassPriority,
            Self::Cast(_)
            | Self::CastWithPayment { .. }
            | Self::CastWithCreatureSpellAdditionalMana { .. }
            | Self::CastWithColorChoice { .. } => CommandKind::Cast,
            Self::CastWithMode { .. } | Self::CastWithModeAndColorChoice { .. } => {
                CommandKind::CastWithMode
            }
            Self::PlayLand { .. } | Self::PlayLandWithEntryLifePayment { .. } => {
                CommandKind::PlayLand
            }
            Self::ActivateManaAbility { .. } => CommandKind::ActivateManaAbility,
            Self::ActivateBoundManaAbility { .. }
            | Self::ActivateBoundManaAbilityWithBundleChoice { .. } => {
                CommandKind::ActivateBoundManaAbility
            }
            Self::ActivateAbility { .. } | Self::ActivateAbilityWithGeneralizedCosts { .. } => {
                CommandKind::ActivateAbility
            }
            Self::Transmute { .. } => CommandKind::Transmute,
            Self::DeclareAttackers { .. } => CommandKind::DeclareAttackers,
            Self::DeclareBlockers { .. } => CommandKind::DeclareBlockers,
            Self::Draw { .. } => CommandKind::Draw,
            Self::ChoosePrivateLibraryCards { .. } => CommandKind::ChoosePrivateLibraryCards,
            Self::ChoosePrivateOpponentLibraryCardToExile { .. } => {
                CommandKind::ChoosePrivateOpponentLibraryCardToExile
            }
            Self::ChooseLibrarySearchCard { .. } => CommandKind::ChooseLibrarySearchCard,
            Self::ChooseTriggeredAbilityTargets { .. } => {
                CommandKind::ChooseTriggeredAbilityTargets
            }
            Self::ChooseTriggeredAbilityEffectObject { .. } => {
                CommandKind::ChooseTriggeredAbilityEffectObject
            }
            Self::ChooseDamageReplacement { .. } => CommandKind::ChooseDamageReplacement,
            Self::ResolveOptionalTriggeredAbility { .. } => {
                CommandKind::ResolveOptionalTriggeredAbility
            }
            Self::SubmitDecision { .. } => CommandKind::SubmitDecision,
            Self::ReportEngineWeakness { .. } => CommandKind::ReportEngineWeakness,
        }
    }
}

/// Mirrors the engine's `PolicyMoveKind` so receipts stay comparable across
/// the boundary.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum CommandKind {
    Cast,
    CastWithMode,
    Draw,
    ChoosePrivateLibraryCards,
    ChoosePrivateOpponentLibraryCardToExile,
    ChooseLibrarySearchCard,
    ChooseTriggeredAbilityTargets,
    ChooseTriggeredAbilityEffectObject,
    ChooseDamageReplacement,
    ResolveOptionalTriggeredAbility,
    SubmitDecision,
    Transmute,
    PassPriority,
    PlayLand,
    ActivateManaAbility,
    ActivateBoundManaAbility,
    ActivateAbility,
    DeclareAttackers,
    DeclareBlockers,
    ReportEngineWeakness,
}

/// One command, fully attributed.
///
/// Every field is required. A command that does not say which match, seat,
/// request, and observed revision it belongs to cannot be safely applied and
/// is not representable here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandEnvelope {
    pub schema: SchemaVersion,
    pub match_id: MatchId,
    /// The seat this command is submitted on behalf of.
    pub seat: SeatId,
    /// The revision the submitter had observed. Must equal the revision the
    /// open request was issued at.
    pub observed_revision: StateRevision,
    /// The request being answered.
    pub request: ActionRequestId,
    /// Client idempotency key.
    pub client_command_id: ClientCommandId,
    pub command: GameCommand,
}

impl CommandEnvelope {
    #[must_use]
    pub fn new(
        match_id: MatchId,
        seat: SeatId,
        observed_revision: StateRevision,
        request: ActionRequestId,
        client_command_id: ClientCommandId,
        command: GameCommand,
    ) -> Self {
        Self {
            schema: crate::version::PROTOCOL_SCHEMA_VERSION,
            match_id,
            seat,
            observed_revision,
            request,
            client_command_id,
            command,
        }
    }
}

/// A receipt for an accepted command.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandReceipt {
    pub match_id: MatchId,
    pub seat: SeatId,
    pub request: ActionRequestId,
    pub client_command_id: ClientCommandId,
    pub kind: CommandKind,
    /// The revision before the command was applied.
    pub previous_revision: StateRevision,
    /// The revision after the command was applied.
    pub revision: StateRevision,
    /// Identity of the controller that produced the command, for attribution.
    pub controller: String,
}

/// The outcome of submitting one envelope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CommandResult {
    Accepted {
        receipt: CommandReceipt,
        /// Events sealed by this transition, already redacted for the
        /// submitting seat.
        events: Vec<crate::event::EventRecord>,
    },
    /// The command was not applied and no state changed.
    ///
    /// `revision` is the current revision, unchanged, so the caller can
    /// re-observe without a separate round trip.
    Rejected {
        error: ProtocolError,
        revision: StateRevision,
    },
    /// This client command id was already applied. The original receipt is
    /// replayed; nothing executed a second time.
    Duplicate { receipt: CommandReceipt },
}

impl CommandResult {
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }

    /// True when this outcome guarantees no state mutation occurred.
    #[must_use]
    pub const fn is_inert(&self) -> bool {
        matches!(self, Self::Rejected { .. } | Self::Duplicate { .. })
    }
}

/// Why a command was refused.
///
/// Every variant here describes a refusal that happens *before* the rules
/// engine is touched, except [`Self::RulesRejected`], which reports an
/// engine refusal that itself left no partial mutation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ProtocolError {
    SchemaMismatch {
        expected: SchemaVersion,
        actual: SchemaVersion,
    },
    UnknownMatch {
        match_id: MatchId,
    },
    /// The envelope names a different match than the request it answers.
    MatchMismatch {
        expected: MatchId,
        actual: MatchId,
    },
    UnknownSeat {
        seat: SeatId,
    },
    /// The submitting seat is not the seat the request was issued to.
    WrongSeat {
        expected: SeatId,
        actual: SeatId,
    },
    /// The submitter observed an older revision than the open request.
    StaleRevision {
        observed: StateRevision,
        current: StateRevision,
    },
    /// The submitter answered a request that is no longer open.
    StaleRequest {
        observed: ActionRequestId,
        current: ActionRequestId,
    },
    /// The command echoes a rules decision identity the request did not open.
    StaleDecision {
        observed: Option<DecisionRef>,
        expected: Option<DecisionRef>,
    },
    /// The command shape does not fit the open request.
    UnexpectedCommand {
        detail: String,
    },
    /// The rules engine refused the action. No state changed.
    RulesRejected {
        detail: String,
    },
    /// A capability the command depends on is not supported by this build.
    Unsupported {
        capability: String,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaMismatch { expected, actual } => {
                write!(
                    formatter,
                    "schema mismatch: expected {expected}, got {actual}"
                )
            }
            Self::UnknownMatch { match_id } => write!(formatter, "unknown match {match_id}"),
            Self::MatchMismatch { expected, actual } => {
                write!(
                    formatter,
                    "command names match {actual}, request is for {expected}"
                )
            }
            Self::UnknownSeat { seat } => write!(formatter, "unknown {seat}"),
            Self::WrongSeat { expected, actual } => {
                write!(
                    formatter,
                    "{actual} acted on a request issued to {expected}"
                )
            }
            Self::StaleRevision { observed, current } => {
                write!(formatter, "stale revision {observed}; current is {current}")
            }
            Self::StaleRequest { observed, current } => {
                write!(formatter, "stale request {observed}; current is {current}")
            }
            Self::StaleDecision { observed, expected } => write!(
                formatter,
                "command echoed decision {observed:?} but the open request is for {expected:?}"
            ),
            Self::UnexpectedCommand { detail } => {
                write!(formatter, "unexpected command: {detail}")
            }
            Self::RulesRejected { detail } => write!(formatter, "rules rejected: {detail}"),
            Self::Unsupported { capability } => {
                write!(formatter, "unsupported capability: {capability}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}
