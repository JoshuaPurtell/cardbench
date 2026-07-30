//! Reference Rust policies for deterministic Magic engine development matches.

#![forbid(unsafe_code)]

mod boros_tempo;
mod development_match;
mod selesnya_convoke;

pub use boros_tempo::BorosTempoPolicy;
pub use development_match::{PolicyMatchResult, run_rav_reference_match};
pub use selesnya_convoke::SelesnyaConvokePolicy;

use cardbench_magic_engine::{GameView, PolicyAction};

/// Submission ABI for `cardbench/magic/code_policy` development runs.
pub trait CodePolicy {
    fn id(&self) -> &'static str;
    fn propose_move(&mut self, view: &GameView) -> PolicyAction;
}
