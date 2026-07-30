//! ReAct agent for Pokemon TCG.
//!
//! This module provides a ReAct-style AI agent that uses an LLM to play Pokemon TCG.
//! It follows the pattern established in the ContinualLearningBench crafter implementation.
//!
//! ## Architecture
//!
//! The module consists of three main components:
//!
//! 1. **Renderer** (`render.rs`): Converts `GameView` to human-readable text for LLM consumption.
//!    - Full game state rendering with `render_game_view()`
//!    - Compact rendering for repeated observations with `render_game_view_compact()`
//!
//! 2. **Action Parser** (`action_parser.rs`): Parses LLM responses into `Action` enums.
//!    - Supports JSON format: `{"action": "PlayBasic", "card_id": 123}`
//!    - Supports natural language: "Play Basic Pokemon with id 123"
//!    - Context-aware parsing based on pending prompts
//!
//! 3. **ReactAi** (`react_ai.rs`): The main AI controller implementing `AiController`.
//!    - Pluggable LLM provider interface
//!    - Strict to random actions on LLM failure
//!    - Action history tracking for analysis
//!    - TODO list maintenance across turns
//!
//! ## Usage
//!
//! ### With a custom LLM provider:
//!
//! ```ignore
//! use tcg_ai::react::{ReactAi, ReactAiConfig, LlmProvider};
//! use std::sync::Arc;
//!
//! struct MyLlmProvider {
//!     api_key: String,
//! }
//!
//! impl LlmProvider for MyLlmProvider {
//!     fn generate(&self, system: &str, user: &str, config: &ReactAiConfig) -> Result<String, String> {
//!         // Call your LLM API here
//!         Ok(r#"{"action": "EndTurn"}"#.to_string())
//!     }
//! }
//!
//! let provider = Arc::new(MyLlmProvider { api_key: "...".to_string() });
//! let ai = ReactAi::with_config(42, ReactAiConfig::default(), provider);
//! ```
//!
//! ### With a closure:
//!
//! ```ignore
//! use tcg_ai::react::ReactAi;
//!
//! let ai = ReactAi::with_llm_fn(42, |system, user, config| {
//!     // Your LLM call here
//!     Ok(r#"{"action": "EndTurn"}"#.to_string())
//! });
//! ```
//!
//! ### As a strict-only random agent:
//!
//! ```ignore
//! use tcg_ai::react::ReactAi;
//!
//! // Without an LLM provider, it will always use random strict
//! let ai = ReactAi::new(42);
//! ```
//!
//! ## Integration with Game Server
//!
//! The `ReactAi` implements `AiController`, so it can be used directly with the game server:
//!
//! ```ignore
//! use tcg_ai::react::ReactAi;
//! use tcg_ai::AiController;
//!
//! let mut ai: Box<dyn AiController + Send> = Box::new(ReactAi::new(42));
//!
//! // When the game has a prompt for this player:
//! let actions = ai.propose_prompt_response(&view, &prompt);
//!
//! // When the game is in Main/Attack phase:
//! let actions = ai.propose_free_actions(&view);
//! ```
//!
//! ## Playing Against AI v4
//!
//! To set up a game where the ReactAi plays against the deterministic AI v4:
//!
//! ```ignore
//! use tcg_ai::{RandomAiV4, AiController};
//! use tcg_ai::react::ReactAi;
//!
//! // Create both AI players
//! let mut react_ai = ReactAi::with_llm_fn(42, my_llm_fn);
//! let mut v4_ai = RandomAiV4::new(123);
//!
//! // In your game loop, dispatch to the appropriate AI based on player ID
//! ```

pub mod render;
pub mod action_parser;
pub mod react_ai;
pub mod runner;

// Re-export main types
pub use render::{render_game_view, render_game_view_compact};
pub use action_parser::{parse_action, parse_response, ParseError, ParsedResponse};
pub use react_ai::{
    ReactAi, ReactAiConfig, LlmProvider, PlaceholderLlmProvider, SyncLlmWrapper,
    ActionHistoryEntry, DEFAULT_SYSTEM_PROMPT,
};
pub use runner::{
    run_game, run_react_vs_v4, create_observation,
    GameResult, GameEndReason, TurnSummary, ActionSummary, RunConfig, StepObservation,
};
