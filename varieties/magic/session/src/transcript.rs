//! A reviewable match transcript: manifest, structured events, JSONL.
//!
//! Modelled on the Pokémon variety's `eventlog.jsonl` plus its `games` /
//! `game_log` split, with one deliberate difference: this one is written from
//! an actual match. The Pokémon eval scripts synthesise the state they emit
//! from the candidate's score (`hp_fraction: max(0.1, candidate_score)`), so
//! the artefact is a picture of the result rather than a trace of the game.
//!
//! JSONL rather than `SQLite`. Their schema is the better long-term shape and
//! this record layout imports into it directly -- `manifest` is one `games`
//! row, each event is one `game_log` row keyed by `sequence` -- but their
//! `SQLite` layer is called only from its own unit tests, and a text format that
//! greps, diffs, and survives in a git-tracked fixture is worth more here than
//! a database nothing reads yet.

use crate::project::{ProjectedEvent, project};
use cardbench_magic_engine::Game;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Everything needed to identify and reproduce one match.
///
/// The fields are exactly the reproduction inputs plus the outcome, so a
/// transcript is self-describing: nothing else has to be recorded to re-run it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchManifest {
    pub schema_version: String,
    /// Deck ids seated as player zero and player one, in that order.
    pub decks: [String; 2],
    /// Pilot identities in seat order, including the policy generation.
    pub policies: [String; 2],
    pub shuffle_seed: u64,
    pub opening_hand_size: u8,
    pub turns: u32,
    /// Winning seat, or `None` for a draw or a non-rules stop.
    pub winner: Option<u16>,
    pub termination: String,
    pub life: [i64; 2],
    pub accepted_policy_moves: u32,
    pub rejected_policy_moves: u32,
    /// Replay digest over the canonical event log.
    pub digest: String,
}

/// A whole match, ready to write or to analyse.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatchTranscript {
    pub manifest: MatchManifest,
    pub events: Vec<ProjectedEvent>,
}

impl MatchTranscript {
    /// Builds a transcript from a finished game plus the run's metadata.
    #[must_use]
    pub fn capture(game: &Game, manifest: MatchManifest) -> Self {
        Self {
            manifest,
            events: project(game),
        }
    }

    /// Serialises to JSON Lines: manifest first, then one event per line.
    ///
    /// Line-oriented so a long match streams, greps, and diffs. The manifest
    /// leads so a reader can decide whether to consume the rest.
    ///
    /// # Errors
    ///
    /// Returns an error only if a record fails to serialise, which would mean
    /// a schema bug rather than a data problem.
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        let mut out = String::new();
        let manifest = serde_json::to_string(&TranscriptLine::Manifest {
            manifest: &self.manifest,
        })?;
        out.push_str(&manifest);
        out.push('\n');
        for event in &self.events {
            let line = serde_json::to_string(&TranscriptLine::Event { event })?;
            out.push_str(&line);
            out.push('\n');
        }
        Ok(out)
    }

    /// Parses a transcript back from JSON Lines.
    ///
    /// # Errors
    ///
    /// Returns an error when a line is malformed or the manifest is absent.
    pub fn from_jsonl(text: &str) -> Result<Self, String> {
        let mut manifest = None;
        let mut events = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed: OwnedTranscriptLine = serde_json::from_str(line)
                .map_err(|error| format!("line {}: {error}", index + 1))?;
            match parsed {
                OwnedTranscriptLine::Manifest { manifest: value } => manifest = Some(value),
                OwnedTranscriptLine::Event { event } => events.push(event),
            }
        }
        Ok(Self {
            manifest: manifest.ok_or_else(|| "transcript has no manifest line".to_owned())?,
            events,
        })
    }

    /// Every event of one kind.
    pub fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a ProjectedEvent> + 'a {
        self.events.iter().filter(move |event| event.kind == kind)
    }

    /// The highest turn number the transcript reaches.
    #[must_use]
    pub fn last_turn(&self) -> u32 {
        self.events
            .iter()
            .map(|event| event.turn)
            .max()
            .unwrap_or(0)
    }

    /// The canonical log, for digest comparison against a fresh replay.
    #[must_use]
    pub fn canonical(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|event| event.canonical.clone())
            .collect()
    }
}

/// Borrowed line shape used when writing.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TranscriptLine<'a> {
    Manifest { manifest: &'a MatchManifest },
    Event { event: &'a ProjectedEvent },
}

/// Owned line shape used when reading.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OwnedTranscriptLine {
    Manifest { manifest: MatchManifest },
    Event { event: ProjectedEvent },
}

/// One turn's worth of activity, derived from the transcript.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TurnSummary {
    pub turn: u32,
    pub active_seat: Option<u16>,
    /// Damage dealt to each seat this turn, indexed by seat.
    pub damage_to_seats: Vec<(u16, i64)>,
    pub spells_cast: Vec<u16>,
    pub lands_played: Vec<u16>,
    pub attackers_declared: Vec<(u16, i64)>,
    pub blocks_declared: Vec<(u16, i64)>,
    pub mana_added: Vec<(u16, i64)>,
}

/// Reduces a transcript to a per-turn timeline.
///
/// This is the view a reviewer wants first: what actually happened, turn by
/// turn, without reading a thousand canonical lines.
#[must_use]
pub fn timeline(transcript: &MatchTranscript) -> Vec<TurnSummary> {
    let mut turns: Vec<TurnSummary> = Vec::new();
    for event in &transcript.events {
        if turns.last().is_none_or(|last| last.turn != event.turn) {
            turns.push(TurnSummary {
                turn: event.turn,
                ..TurnSummary::default()
            });
        }
        let Some(current) = turns.last_mut() else {
            continue;
        };
        let seat = event.facts.seats.first().map(|seat| seat.0);
        match event.kind.as_str() {
            "StepBegan" => current.active_seat = seat,
            "DamageDealtToPlayer" => {
                if let (Some(seat), Some(amount)) = (seat, event.facts.amount) {
                    accumulate(&mut current.damage_to_seats, seat, amount);
                }
            }
            "SpellCast" => {
                if let Some(seat) = seat {
                    current.spells_cast.push(seat);
                }
            }
            "ManaAdded" => {
                if let (Some(seat), Some(amount)) = (seat, event.facts.amount) {
                    accumulate(&mut current.mana_added, seat, amount);
                }
            }
            "AttackersDeclared" => {
                if let (Some(seat), Some(amount)) = (seat, event.facts.amount) {
                    current.attackers_declared.push((seat, amount));
                }
            }
            "BlockersDeclared" => {
                if let (Some(seat), Some(amount)) = (seat, event.facts.amount) {
                    current.blocks_declared.push((seat, amount));
                }
            }
            "PolicyMoveSubmitted" => {
                if event.facts.move_kind.as_deref() == Some("PlayLand")
                    && let Some(seat) = seat
                {
                    current.lands_played.push(seat);
                }
            }
            _ => {}
        }
    }
    turns
}

fn accumulate(entries: &mut Vec<(u16, i64)>, seat: u16, amount: i64) {
    if let Some(entry) = entries.iter_mut().find(|(existing, _)| *existing == seat) {
        entry.1 += amount;
    } else {
        entries.push((seat, amount));
    }
}

/// Renders a compact human-readable timeline.
#[must_use]
pub fn render_timeline(transcript: &MatchTranscript) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} vs {}  seed={}  {} turns  digest={}",
        transcript.manifest.decks[0],
        transcript.manifest.decks[1],
        transcript.manifest.shuffle_seed,
        transcript.manifest.turns,
        transcript.manifest.digest,
    );
    let _ = writeln!(
        out,
        "pilots: seat0={} seat1={}",
        transcript.manifest.policies[0], transcript.manifest.policies[1]
    );
    let _ = writeln!(
        out,
        "{:>5}  {:>4}  {:>5}  {:>5}  {:>5}  {:>5}  damage",
        "turn", "seat", "mana", "spell", "land", "atk"
    );
    for turn in timeline(transcript) {
        if turn.turn == 0 {
            continue;
        }
        let damage: String = turn
            .damage_to_seats
            .iter()
            .map(|(seat, amount)| format!("s{seat}:{amount}"))
            .collect::<Vec<_>>()
            .join(" ");
        let mana: i64 = turn.mana_added.iter().map(|(_, amount)| *amount).sum();
        let attackers: i64 = turn
            .attackers_declared
            .iter()
            .map(|(_, count)| *count)
            .sum();
        let _ = writeln!(
            out,
            "{:>5}  {:>4}  {:>5}  {:>5}  {:>5}  {:>5}  {}",
            turn.turn,
            turn.active_seat
                .map_or_else(|| "-".to_owned(), |seat| seat.to_string()),
            mana,
            turn.spells_cast.len(),
            turn.lands_played.len(),
            attackers,
            damage,
        );
    }
    out
}
