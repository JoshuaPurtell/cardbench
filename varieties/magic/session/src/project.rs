//! Engine events projected into structured, serialisable records.
//!
//! This is the piece the Pokémon variety gets right and Magic did not: its
//! `GameEvent` derives serde, so a log is JSON and a reviewer can query it.
//! Magic's engine cannot do the same, and should not — the engine, policies,
//! and expansion crates are deliberately dependency-free, and the architecture
//! contract confines serde to the protocol layer.
//!
//! So the adaptation is a projection rather than a derive. Every engine event
//! becomes a record with:
//!
//! * a **typed fact set** for the events a reviewer actually queries -- who
//!   acted, which objects, how much, which policy -- so no consumer has to
//!   regex a `Debug` rendering, and
//! * the **canonical string** for every event regardless, so fidelity is never
//!   traded for convenience and the replay digest stays comparable.
//!
//! The typed half covers a couple of dozen variants out of the engine's 112.
//! That is deliberate: the rest are still recorded and still replayable, they
//! just are not yet queryable by field. Extending coverage is additive.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_protocol::{EventRecord, ObjectRef, SeatId, StateRevision};
use serde::{Deserialize, Serialize};

/// Structured facts extracted from one engine event.
///
/// Every field is optional because events differ; a reviewer filters on the
/// ones present. This is not a second event vocabulary -- it is an index over
/// the canonical one.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventFacts {
    /// Seats the event names, in the order the event names them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seats: Vec<SeatId>,
    /// Objects the event names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<ObjectRef>,
    /// A scalar quantity: damage, life, mana.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<i64>,
    /// The policy identity that submitted a move. This is the field with no
    /// counterpart in the Pokémon log, and the one that makes a transcript
    /// attributable to a specific pilot generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
    /// The move kind for a submission receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_kind: Option<String>,
    /// Destination zone for a zone change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zone: Option<String>,
    /// Step, for a step transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
}

impl EventFacts {
    fn seat(player: PlayerId) -> Vec<SeatId> {
        vec![seat_of(player)]
    }
}

/// One projected event: canonical text plus queryable facts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectedEvent {
    /// Position in the authoritative stream, and the replay cursor.
    pub sequence: u64,
    /// Turn this event occurred on, carried forward from the last step
    /// transition so every record is placeable in time without a join.
    pub turn: u32,
    /// The engine event variant name.
    pub kind: String,
    #[serde(default, skip_serializing_if = "is_default_facts")]
    pub facts: EventFacts,
    /// The exact engine replay line. Always present: the digest is computed
    /// over these, so dropping one would make a transcript unverifiable.
    pub canonical: String,
}

fn is_default_facts(facts: &EventFacts) -> bool {
    facts == &EventFacts::default()
}

#[must_use]
fn seat_of(player: PlayerId) -> SeatId {
    SeatId(u16::try_from(player.0).unwrap_or(u16::MAX))
}

#[must_use]
fn object_of(object: cardbench_magic_engine::ObjectId) -> ObjectRef {
    ObjectRef(object.0)
}

fn variant_name(event: &GameEvent) -> String {
    let rendered = format!("{event:?}");
    rendered
        .split_once([' ', '{', '('])
        .map_or_else(|| rendered.clone(), |(head, _)| head.to_owned())
}

/// Extracts the typed facts for one event.
///
/// Falls through to empty facts for variants not yet covered; the canonical
/// line still carries everything, so nothing is lost, only un-indexed.
#[must_use]
#[allow(clippy::too_many_lines)] // One flat table of event shapes reads better than a split one.
pub fn facts_for(event: &GameEvent) -> EventFacts {
    match event {
        GameEvent::PolicyMoveSubmitted {
            player,
            policy,
            kind,
        } => EventFacts {
            seats: EventFacts::seat(*player),
            policy: Some(policy.clone()),
            move_kind: Some(format!("{kind:?}")),
            ..EventFacts::default()
        },
        GameEvent::StepBegan {
            turn,
            active_player,
            step,
        } => EventFacts {
            seats: EventFacts::seat(*active_player),
            amount: Some(i64::from(*turn)),
            step: Some(step_name(*step)),
            ..EventFacts::default()
        },
        GameEvent::DamageDealtToPlayer {
            source,
            player,
            amount,
        } => EventFacts {
            seats: EventFacts::seat(*player),
            objects: vec![object_of(*source)],
            amount: Some(i64::from(*amount)),
            ..EventFacts::default()
        },
        GameEvent::DamageDealtToPermanent {
            source,
            permanent,
            amount,
        } => EventFacts {
            objects: vec![object_of(*source), object_of(*permanent)],
            amount: Some(i64::from(*amount)),
            ..EventFacts::default()
        },
        GameEvent::LifeGained { player, amount } => EventFacts {
            seats: EventFacts::seat(*player),
            amount: Some(i64::from(*amount)),
            ..EventFacts::default()
        },
        GameEvent::CardMoved { card, to } => EventFacts {
            objects: vec![object_of(*card)],
            zone: Some(zone_name(*to)),
            ..EventFacts::default()
        },
        GameEvent::SpellCast { player, card } => EventFacts {
            seats: EventFacts::seat(*player),
            objects: vec![object_of(*card)],
            ..EventFacts::default()
        },
        GameEvent::SpellResolved { card } => EventFacts {
            objects: vec![object_of(*card)],
            ..EventFacts::default()
        },
        GameEvent::AttackersDeclared { player, attackers } => EventFacts {
            seats: EventFacts::seat(*player),
            objects: attackers.iter().copied().map(object_of).collect(),
            amount: Some(i64::try_from(attackers.len()).unwrap_or(i64::MAX)),
            ..EventFacts::default()
        },
        GameEvent::BlockersDeclared {
            player,
            assignments,
        } => EventFacts {
            seats: EventFacts::seat(*player),
            objects: assignments
                .iter()
                .flat_map(|(attacker, blocker)| [object_of(*attacker), object_of(*blocker)])
                .collect(),
            amount: Some(i64::try_from(assignments.len()).unwrap_or(i64::MAX)),
            ..EventFacts::default()
        },
        GameEvent::ManaAdded {
            player,
            color,
            amount,
        } => EventFacts {
            seats: EventFacts::seat(*player),
            amount: Some(i64::from(*amount)),
            zone: Some(format!("{color:?}")),
            ..EventFacts::default()
        },
        GameEvent::CardDestroyed { source, card } => EventFacts {
            objects: vec![object_of(*source), object_of(*card)],
            ..EventFacts::default()
        },
        GameEvent::TokenCeasedToExist { token } => EventFacts {
            objects: vec![object_of(*token)],
            ..EventFacts::default()
        },
        GameEvent::PriorityPassed { player } => EventFacts {
            seats: EventFacts::seat(*player),
            ..EventFacts::default()
        },
        GameEvent::PlayerLost { player, reason } => EventFacts {
            seats: EventFacts::seat(*player),
            move_kind: Some((*reason).to_owned()),
            ..EventFacts::default()
        },
        GameEvent::GameEnded { winner } => EventFacts {
            seats: winner.map(EventFacts::seat).unwrap_or_default(),
            ..EventFacts::default()
        },
        _ => EventFacts::default(),
    }
}

fn step_name(step: Step) -> String {
    format!("{step:?}")
}

fn zone_name(zone: Zone) -> String {
    format!("{zone:?}")
}

/// Projects a game's whole event log into structured records.
///
/// Turn numbers are carried forward from step transitions so every record is
/// placeable without a join back into the stream.
#[must_use]
pub fn project(game: &Game) -> Vec<ProjectedEvent> {
    project_events(&game.event_log)
}

/// Projects a bare event slice.
///
/// Separate from [`project`] so a runner that already owns the typed log does
/// not have to keep the whole `Game` alive to produce a transcript.
#[must_use]
pub fn project_events(events: &[GameEvent]) -> Vec<ProjectedEvent> {
    let mut turn = 0_u32;
    events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            if let GameEvent::StepBegan { turn: began, .. } = event {
                turn = *began;
            }
            ProjectedEvent {
                sequence: u64::try_from(index).unwrap_or(u64::MAX),
                turn,
                kind: variant_name(event),
                facts: facts_for(event),
                canonical: format!("{event:?}"),
            }
        })
        .collect()
}

/// Projects into the protocol's viewer-redactable record type.
///
/// Used when a transcript will be shown to a seat rather than to a reviewer:
/// [`EventRecord`] carries the visibility rules and the redaction that
/// [`ProjectedEvent`] deliberately does not, because a reviewer is an
/// authority scope and should see everything.
#[must_use]
pub fn project_for_transport(game: &Game) -> Vec<EventRecord> {
    project(game)
        .into_iter()
        .map(|event| {
            EventRecord::public(
                event.sequence,
                StateRevision::new(event.sequence, game.public_state_digest()),
                event.kind,
                event.facts.seats,
                event.canonical,
            )
        })
        .collect()
}
