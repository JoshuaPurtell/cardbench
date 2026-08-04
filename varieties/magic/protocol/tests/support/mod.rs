//! Deterministic fixtures shared by the protocol contract tests.
//!
//! These build protocol values directly rather than projecting from a game.
//! That is intentional: this crate has no engine dependency, and the contracts
//! under test here are properties of the schema itself. Projection fidelity is
//! the session crate's responsibility and is tested against real games there.

#![allow(dead_code)]

use cardbench_magic_protocol::{
    ActionRequest, ActionRequestKind, CardDto, CardName, ClientCommandId, CombatObservation,
    CommandEnvelope, GameCommand, HiddenZoneProjection, LegalActionSurface, MatchId,
    MatchObservation, ObjectRef, PROTOCOL_SCHEMA_VERSION, SeatId, SeatObservation, StateRevision,
    StepDto, TeamId, TeamObservation, ViewerScope,
};
use std::collections::BTreeSet;

pub const SEAT_A: SeatId = SeatId(0);
pub const SEAT_B: SeatId = SeatId(1);
pub const SEAT_C: SeatId = SeatId(2);

pub fn match_id() -> MatchId {
    MatchId::new("contract-fixture")
}

pub fn revision(sequence: u64) -> StateRevision {
    StateRevision::new(sequence, 0xdead_beef_0000_0000 ^ sequence)
}

/// A fully identified card. Every characteristic is populated so a redaction
/// test that only checks the name would still see the other fields survive.
pub fn card(object: u64, name: &str, controller: SeatId) -> CardDto {
    use cardbench_magic_protocol::{CardTypeDto, ColorDto};

    CardDto {
        object: ObjectRef(object),
        definition: Some(CardName::new(name)),
        controller,
        tapped: false,
        colors: BTreeSet::from([ColorDto::Green]),
        mana_colors: BTreeSet::new(),
        basic_land_type: None,
        card_types: BTreeSet::from([CardTypeDto::Creature]),
        can_attack: true,
    }
}

/// A seat holding two cards in hand and three in library, all identified, plus
/// one permanent on the battlefield.
pub fn seat(seat: SeatId, team: TeamId, base_object: u64) -> SeatObservation {
    use cardbench_magic_protocol::{ColorDto, ManaAmountDto, ManaPoolDto};

    SeatObservation {
        seat,
        team,
        life: 20,
        eliminated: false,
        lands_played: 1,
        battlefield: vec![card(base_object, "Battlefield Bear", seat)],
        graveyard: vec![card(base_object + 1, "Dead Bear", seat)],
        exile: Vec::new(),
        hand: HiddenZoneProjection::Revealed(vec![
            card(base_object + 2, "Hidden Bolt", seat),
            card(base_object + 3, "Hidden Counterspell", seat),
        ]),
        library: HiddenZoneProjection::Revealed(vec![
            card(base_object + 4, "Library Top", seat),
            card(base_object + 5, "Library Middle", seat),
            card(base_object + 6, "Library Bottom", seat),
        ]),
        revealed_library_top: None,
        mana_pool: Some(ManaPoolDto {
            amounts: vec![ManaAmountDto {
                color: ColorDto::Green,
                amount: 2,
            }],
        }),
    }
}

/// A two-seat authoritative observation: every seat's hidden zones revealed,
/// as only a judge scope may legitimately hold.
pub fn authoritative_observation() -> MatchObservation {
    MatchObservation {
        schema: PROTOCOL_SCHEMA_VERSION,
        match_id: match_id(),
        revision: revision(7),
        viewer: ViewerScope::Judge,
        turn: 3,
        step: StepDto::PrecombatMain,
        active_seat: SEAT_A,
        priority_seat: SEAT_A,
        decision_seat: SEAT_A,
        seats: vec![seat(SEAT_A, TeamId(0), 100), seat(SEAT_B, TeamId(1), 200)],
        teams: vec![
            TeamObservation {
                team: TeamId(0),
                seats: vec![SEAT_A],
                eliminated: false,
            },
            TeamObservation {
                team: TeamId(1),
                seats: vec![SEAT_B],
                eliminated: false,
            },
        ],
        stack: Vec::new(),
        combat: CombatObservation::default(),
        terminal: None,
    }
}

/// A three-seat authoritative observation with one seat already eliminated.
///
/// An eliminated seat stays in the roster: its objects and event provenance
/// remain addressable, which is the property a pod needs and a duel never
/// exercises.
pub fn three_seat_observation() -> MatchObservation {
    let mut observation = authoritative_observation();
    let mut third = seat(SEAT_C, TeamId(2), 300);
    third.eliminated = true;
    third.life = 0;
    observation.seats.push(third);
    observation.teams.push(TeamObservation {
        team: TeamId(2),
        seats: vec![SEAT_C],
        eliminated: true,
    });
    observation
}

pub fn priority_request(seat: SeatId, revision: StateRevision, id: u64) -> ActionRequest {
    use cardbench_magic_protocol::ActionRequestId;

    let mut observation = authoritative_observation();
    observation.revision = revision;
    observation.viewer = ViewerScope::Seat(seat);
    ActionRequest {
        schema: PROTOCOL_SCHEMA_VERSION,
        match_id: match_id(),
        id: ActionRequestId(id),
        seat,
        revision,
        kind: ActionRequestKind::Priority,
        decision: None,
        observation,
        legal: LegalActionSurface::unavailable(),
    }
}

pub fn envelope(request: &ActionRequest, id: &str, command: GameCommand) -> CommandEnvelope {
    request.reply(ClientCommandId::new(id), command)
}

pub fn pass(request: &ActionRequest, id: &str) -> CommandEnvelope {
    envelope(request, id, GameCommand::PassPriority)
}
