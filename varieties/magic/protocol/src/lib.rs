//! Versioned, owned, transport-neutral game protocol for the `CardBench`
//! Magic variety.
//!
//! # What this crate is
//!
//! The structured interface between a Magic match and anything outside it: a
//! local policy, a scripted test controller, a human client, and eventually a
//! network transport. It defines identities, observations, requests, commands,
//! results, events, and a capability manifest, all owned and serialisable.
//!
//! # What this crate is deliberately not
//!
//! It does not depend on the rules engine, and it must not. That single edge
//! being absent is what structurally guarantees the invariant that transport
//! code cannot grow a second rules or legality implementation: there is
//! nothing here to check legality *with*. Projection from the engine into
//! these types, and submission from these types into the engine, both live in
//! the session crate, on the engine side of the boundary.
//!
//! It also contains no networking. Milestone 1 proves the contract in-process.
//!
//! # The four invariants this crate carries
//!
//! - **Nothing stale mutates a later state.** [`validate::validate_envelope`]
//!   is a total function over owned data and rejects on request, revision,
//!   seat, match, and decision identity before any engine call is possible.
//! - **Every accepted command is attributable.** [`CommandEnvelope`] cannot be
//!   constructed without a match, seat, revision, request, and client id, and
//!   [`CommandReceipt`] carries all of them plus the controller.
//! - **Hidden information is projected, not filtered downstream.**
//!   [`HiddenZoneProjection`] makes "count only" and "contents" different
//!   shapes rather than a nullable field, and [`ViewerScope::may_see_private`]
//!   is the one entitlement predicate.
//! - **Claims are explicit.** [`ProtocolCapabilities`] reports schema-modelled
//!   but unimplemented shapes as [`SupportLevel::Modelled`], never supported.

pub mod command;
pub mod event;
pub mod ids;
pub mod observation;
pub mod primitives;
pub mod request;
pub mod scope;
pub mod terminal;
pub mod validate;
pub mod version;

pub use command::{
    AbilityActivationDto, AbilityCostPaymentDto, BlockAssignmentDto, CastRequestDto,
    CommandEnvelope, CommandKind, CommandReceipt, CommandResult, ConvokeContributionDto,
    ConvokePaymentDto, DecisionSelectionDto, ExtraManaPaymentDto, GameCommand,
    ManaAbilityActivationDto, ManaPaymentSelectionDto, PaymentManaAbilityDto, ProtocolError,
};
pub use event::{EventRecord, EventStreamPage, EventVisibility};
pub use ids::{
    AbilityName, ActionRequestId, CardName, ClientCommandId, DecisionRef, MatchId, ObjectRef,
    SeatId, StackObjectRef, StateRevision, TeamId,
};
pub use observation::{
    AttackDeclaration, BlockDeclaration, CombatObservation, DefenderDto, HiddenZoneProjection,
    MatchObservation, SeatObservation, StackItemKind, StackItemObservation, TeamObservation,
};
pub use primitives::{
    BasicLandTypeDto, CardDto, CardTypeDto, ColorDto, CounterKindDto, DamageReplacementChoiceDto,
    HybridManaSymbolDto, ManaAmountDto, ManaBundleDto, ManaCostDto, ManaPoolDto,
    ReplacementChoiceDto, ReplacementEffectDto, StepDto, TargetDto, TriggerOrderEntryDto, ZoneDto,
};
pub use request::{
    ActionRequest, ActionRequestKind, DecisionKindLabel, DecisionPrompt, DecisionVisibilityDto,
    LegalActionOption, LegalActionSurface,
};
pub use scope::{ScopeMembership, ViewerScope};
pub use terminal::{TerminalOutcome, TerminalResult};
pub use validate::{CommandLedger, SubmissionContext, admit, validate_envelope, validate_schema};
pub use version::{
    FeatureCapability, FormatCapability, PROTOCOL_NAME, PROTOCOL_SCHEMA_VERSION,
    ProtocolCapabilities, SchemaVersion, SupportLevel,
};
