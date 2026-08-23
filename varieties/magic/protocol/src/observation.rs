//! What one viewer is entitled to know about a match right now.
//!
//! The shape here is seat-indexed from the start. There is no "own" and
//! "opponent" split and no flattened opposing battlefield: a three- or
//! four-seat match projects through exactly the same structure as a duel,
//! and a client that renders a duel correctly renders a pod correctly.

use crate::ids::{MatchId, ObjectRef, SeatId, StackObjectRef, StateRevision, TeamId};
use crate::primitives::{CardDto, ManaPoolDto, StepDto, TargetDto};
use crate::scope::{ScopeMembership, ViewerScope};
use crate::terminal::TerminalResult;
use crate::version::SchemaVersion;
use serde::{Deserialize, Serialize};

/// A hidden-zone projection: either a count or the actual contents.
///
/// Modelling this as a sum type rather than an optional vector plus a
/// separate count means an unentitled viewer cannot receive both, and a
/// forgotten branch fails to compile instead of leaking.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HiddenZoneProjection {
    /// The viewer knows only how many cards are here.
    Count(u32),
    /// The viewer is entitled to the ordered contents.
    Revealed(Vec<CardDto>),
}

impl HiddenZoneProjection {
    #[must_use]
    pub fn count(&self) -> u32 {
        match self {
            Self::Count(count) => *count,
            Self::Revealed(cards) => u32::try_from(cards.len()).unwrap_or(u32::MAX),
        }
    }

    /// The contents, or `None` when only a count is entitled.
    #[must_use]
    pub const fn revealed(&self) -> Option<&Vec<CardDto>> {
        match self {
            Self::Revealed(cards) => Some(cards),
            Self::Count(_) => None,
        }
    }

    /// Collapses to a count. Idempotent, so it is safe to apply defensively.
    #[must_use]
    pub fn redacted(&self) -> Self {
        Self::Count(self.count())
    }
}

/// One seat's publicly visible state, plus whatever private state the viewer
/// is entitled to.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SeatObservation {
    pub seat: SeatId,
    pub team: TeamId,
    pub life: i64,
    /// True once the seat has lost. The seat stays in the roster so its
    /// historical provenance and any still-relevant objects remain
    /// addressable.
    pub eliminated: bool,
    pub lands_played: u8,
    /// Public zones.
    pub battlefield: Vec<CardDto>,
    pub graveyard: Vec<CardDto>,
    pub exile: Vec<CardDto>,
    /// Hidden zones, projected per the viewer's entitlement.
    pub hand: HiddenZoneProjection,
    pub library: HiddenZoneProjection,
    /// Present only when a live public effect reveals this seat's library top.
    pub revealed_library_top: Option<CardDto>,
    /// Present only for a seat whose private state the viewer may see.
    pub mana_pool: Option<ManaPoolDto>,
}

impl SeatObservation {
    /// Removes every private field this seat exposes.
    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            seat: self.seat,
            team: self.team,
            life: self.life,
            eliminated: self.eliminated,
            lands_played: self.lands_played,
            battlefield: self.battlefield.clone(),
            graveyard: self.graveyard.clone(),
            exile: self.exile.clone(),
            hand: self.hand.redacted(),
            library: self.library.redacted(),
            revealed_library_top: self.revealed_library_top.clone(),
            mana_pool: None,
        }
    }
}

/// A team's identity and membership. Shared life is deliberately absent: no
/// shared-life format is implemented, and a field here would read as a claim.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TeamObservation {
    pub team: TeamId,
    pub seats: Vec<SeatId>,
    /// True once every member seat has been eliminated.
    pub eliminated: bool,
}

/// One item on the stack.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StackItemObservation {
    pub stack_object: StackObjectRef,
    pub controller: SeatId,
    pub kind: StackItemKind,
    /// The physical card, for a spell. An activated ability names no card.
    pub card: Option<CardDto>,
    pub ability: Option<crate::ids::AbilityName>,
    pub targets: Vec<TargetDto>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StackItemKind {
    Spell,
    ActivatedAbility,
    TriggeredAbility,
}

/// What one attacker is attacking.
///
/// This is per-attacker from the outset even though the rules engine
/// currently derives a single defender for the whole combat. The engine's
/// limitation is reported through
/// [`crate::FeatureCapability::ChosenAttackDefender`]; it is not baked into
/// the schema, so removing it later does not break clients.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DefenderDto {
    Seat(SeatId),
    Team(TeamId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AttackDeclaration {
    pub attacker: ObjectRef,
    pub controller: SeatId,
    pub defender: DefenderDto,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct BlockDeclaration {
    pub attacker: ObjectRef,
    pub blocker: ObjectRef,
    pub blocker_controller: SeatId,
    /// Position in the attacking seat's damage-assignment order, when that
    /// order has been submitted. `None` while it is still pending.
    pub damage_order: Option<u8>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CombatObservation {
    pub attackers_declared: bool,
    pub blockers_declared: bool,
    pub attacks: Vec<AttackDeclaration>,
    pub blocks: Vec<BlockDeclaration>,
}

/// Everything a viewer is entitled to know at one revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchObservation {
    pub schema: SchemaVersion,
    pub match_id: MatchId,
    pub revision: StateRevision,
    pub viewer: ViewerScope,
    pub turn: u32,
    pub step: StepDto,
    pub active_seat: SeatId,
    pub priority_seat: SeatId,
    /// The seat that must supply the next decision. It differs from
    /// `priority_seat` for turn-based declarations and mandatory draw
    /// replacements, so a client must never derive one from the other.
    pub decision_seat: SeatId,
    /// Every seat, in seat order, including eliminated ones.
    pub seats: Vec<SeatObservation>,
    pub teams: Vec<TeamObservation>,
    pub stack: Vec<StackItemObservation>,
    pub combat: CombatObservation,
    /// Set once the match has ended.
    pub terminal: Option<TerminalResult>,
}

impl MatchObservation {
    #[must_use]
    pub fn seat(&self, seat: SeatId) -> Option<&SeatObservation> {
        self.seats.iter().find(|entry| entry.seat == seat)
    }

    /// Every seat other than `seat` that is still in the match.
    ///
    /// Returned as identified seats rather than a flattened aggregate so a
    /// caller cannot accidentally treat a pod as a duel.
    #[must_use]
    pub fn living_opponents_of(&self, seat: SeatId) -> Vec<&SeatObservation> {
        self.seats
            .iter()
            .filter(|entry| entry.seat != seat && !entry.eliminated)
            .collect()
    }

    /// Re-projects this observation for a narrower scope.
    ///
    /// This is a defence-in-depth boundary, not the primary one: a projection
    /// should already build only what the scope is entitled to. Applying it
    /// to an authoritative observation is how a spectator or opposing-seat
    /// stream is derived from one authoritative build.
    #[must_use]
    pub fn redacted_for(&self, viewer: ViewerScope, membership: &ScopeMembership) -> Self {
        let seats = self
            .seats
            .iter()
            .map(|seat| {
                if viewer.may_see_private(seat.seat, membership) {
                    seat.clone()
                } else {
                    seat.redacted()
                }
            })
            .collect();
        Self {
            schema: self.schema,
            match_id: self.match_id.clone(),
            revision: self.revision,
            viewer,
            turn: self.turn,
            step: self.step,
            active_seat: self.active_seat,
            priority_seat: self.priority_seat,
            decision_seat: self.decision_seat,
            seats,
            teams: self.teams.clone(),
            stack: self.stack.clone(),
            combat: self.combat.clone(),
            terminal: self.terminal.clone(),
        }
    }

    /// Every card identity this observation discloses, for audit tests.
    ///
    /// Public zones are included because a redaction test needs to assert
    /// what is *absent*, and doing that against a whole-observation scan is
    /// stronger than checking the fields a test author happened to think of.
    #[must_use]
    pub fn disclosed_objects(&self) -> Vec<ObjectRef> {
        let mut disclosed = Vec::new();
        for seat in &self.seats {
            for card in seat
                .battlefield
                .iter()
                .chain(&seat.graveyard)
                .chain(&seat.exile)
                .chain(seat.revealed_library_top.as_slice())
            {
                if card.is_identified() {
                    disclosed.push(card.object);
                }
            }
            for zone in [&seat.hand, &seat.library] {
                if let Some(cards) = zone.revealed() {
                    disclosed.extend(
                        cards
                            .iter()
                            .filter(|card| card.is_identified())
                            .map(|card| card.object),
                    );
                }
            }
        }
        for item in &self.stack {
            if let Some(card) = &item.card
                && card.is_identified()
            {
                disclosed.push(card.object);
            }
        }
        disclosed.sort_unstable();
        disclosed.dedup();
        disclosed
    }
}
