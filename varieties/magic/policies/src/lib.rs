//! Reference Rust policies for deterministic Magic engine development matches.

#![forbid(unsafe_code)]

mod boros_char_control;
mod boros_tempo;
mod deck_match;
mod development_match;
mod selesnya_convoke;
mod selesnya_siege;

pub use boros_char_control::BorosCharControlPolicy;
pub use boros_tempo::BorosTempoPolicy;
pub use deck_match::{
    DeckMatchConfig, DeckMatchResult, DeckMatchSweepResult, DeckMatchTermination, EngineFinding,
    EngineFindingKind, EngineTournamentFailure, EngineTournamentResult, RAV_DECK_MATCH_ID,
    run_rav_deck_matchup, run_rav_engine_tournament, run_rav_full_deck_match,
    run_rav_full_deck_sweep,
};
pub use development_match::{PolicyMatchResult, run_rav_reference_match};
pub use selesnya_convoke::SelesnyaConvokePolicy;
pub use selesnya_siege::SelesnyaSiegePolicy;

use cardbench_magic_engine::{GameView, PolicyAction};

/// Submission ABI for `cardbench/magic/code_policy` development runs.
pub trait CodePolicy {
    fn id(&self) -> &'static str;
    fn propose_move(&mut self, view: &GameView) -> PolicyAction;
}
