use tcg_core::{Action, GameView, Prompt};

/// Option-A AI interface: consumes only the player's `GameView` (+ prompt) and emits actions.
pub trait AiController {
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action>;
    fn propose_free_actions(&mut self, view: &GameView) -> Vec<Action>;
    /// Prompt kinds the controller could not answer. A match loop must report
    /// these as dropped matches rather than falling back to `EndTurn`.
    fn unanswered_prompts(&self) -> Vec<String> {
        Vec::new()
    }
}
