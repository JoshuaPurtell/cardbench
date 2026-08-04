//! The arena: a container that seats code policies and LLM agents on the same
//! decks, under the same seeds, and reports the result the same way.
//!
//! `varieties/magic/policies` answers "is this policy better than that one".
//! It answers it well, and it answers it only for policies written in Rust
//! against `CodePolicy`. This crate exists so the same question can be asked of
//! a model.
//!
//! The design turns on one measurement. A real 38-turn match takes **1025
//! policy decisions** -- Magic has priority and Pokémon does not -- so a seat
//! that consults a model on every decision is not affordable and never will be.
//! Almost all of those decisions have exactly one legal reply. [`menu`] finds
//! the ones that do not, [`probe`] measures how few they are, and [`react`]
//! consults the model only there.
//!
//! Everything here is arranged so an agent seat is an ordinary `CodePolicy`.
//! That is not a stylistic choice: it is what makes `rav-policy-ladder`, the
//! archetype matrix, paired seeds, Wilson intervals, the seat split, and
//! contamination reporting apply to a model seat without a line of new
//! statistics code.
//!
//! ```text
//! menu     GameView            -> the legal options, unscored
//! render   options             -> the prompt an agent sees
//! parse    a reply             -> one option index
//! provider a prompt            -> a reply
//! react    all of the above    -> a CodePolicy
//! runner   two seats           -> a paired, seat-split, interval-bounded result
//! ```

#![forbid(unsafe_code)]

pub mod config;
pub mod menu;
pub mod parse;
pub mod probe;
pub mod provider;
pub mod react;
pub mod render;
pub mod runner;

pub use menu::{Candidate, Menu, Occasion};
pub use provider::{LlmProvider, ProviderError, ScriptedProvider};
pub use react::{ReactConfig, ReactPolicy, ReactStats};
pub use runner::{ArenaResult, DeckOutcome, SeatSpec};
