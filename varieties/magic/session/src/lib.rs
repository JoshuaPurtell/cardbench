//! Engine-to-protocol projection and reviewable match transcripts.
//!
//! Layer C of the architecture contract, landing its first responsibility:
//! turning what the rules engine recorded into something outside the engine
//! can read, query, store, and criticise.
//!
//! # Why this crate exists rather than a derive on the engine
//!
//! The Pokémon variety solves the same problem by deriving serde on its
//! `GameEvent`. Magic cannot copy that directly: the engine, policies, and
//! expansion crates are deliberately dependency-free, and the architecture
//! contract confines serde to the layers above. Projecting instead of deriving
//! keeps that boundary and buys two things a derive would not:
//!
//! * the transcript schema versions independently of the engine's internal
//!   event vocabulary, and
//! * the same projection can feed a reviewer (everything visible) or a seat
//!   (redacted through [`cardbench_magic_protocol::EventRecord`]) without the
//!   engine knowing either exists.
//!
//! # What is adopted from the Pokémon design, and what is not
//!
//! Adopted: structured serialisable events, a manifest-plus-log split
//! mirroring their `games` / `game_log` tables, and a line-oriented file a
//! tool can stream.
//!
//! Not adopted: full mid-game state snapshots. Their `GameStateSnapshot`
//! serialises the entire game including RNG state, which allows forking a
//! match at an arbitrary point. Magic's contract forbids serialising the
//! internal `Game`, and the same capability is reachable without it --
//! matches are deterministic in `(seed, decks, pilots)`, so a fork is a replay
//! with a different pilot swapped in at the branch turn. That is slower and
//! strictly more honest: nothing can resume from a state the rules engine
//! could not have produced.

pub mod critique;
pub mod project;
pub mod transcript;

pub use critique::{Critique, Finding, Severity, critique};
pub use project::{
    EventFacts, ProjectedEvent, facts_for, project, project_events, project_for_transport,
};
pub use transcript::{MatchManifest, MatchTranscript, TurnSummary, render_timeline, timeline};

/// Schema identifier written into every transcript manifest.
pub const TRANSCRIPT_SCHEMA: &str = "cardbench.magic.transcript.v1";
