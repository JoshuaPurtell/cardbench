//! Game runner for ReactAi vs deterministic AI matches.
//!
//! This module provides utilities for running headless matches between
//! a ReactAi agent and deterministic AI opponents (like RandomAiV4).

use tcg_core::{
    GameState, PlayerId, StepResult,
};
use crate::traits::AiController;
use crate::RandomAiV4;
use super::react_ai::ReactAi;
use super::render::render_game_view_compact;

/// Result of a completed game.
#[derive(Debug, Clone)]
pub struct GameResult {
    /// Winner of the game (None if draw/incomplete).
    pub winner: Option<PlayerId>,
    /// Total number of turns played.
    pub turns: u32,
    /// Total number of steps executed.
    pub steps: u32,
    /// History of game states (compact summaries).
    pub history: Vec<TurnSummary>,
    /// Final prizes remaining for P1.
    pub p1_prizes_remaining: usize,
    /// Final prizes remaining for P2.
    pub p2_prizes_remaining: usize,
    /// How the game ended.
    pub end_reason: GameEndReason,
}

/// How the game ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEndReason {
    /// All prizes taken.
    PrizesTaken,
    /// Opponent has no Pokemon in play.
    NoPokemon,
    /// Opponent decked out.
    DeckOut,
    /// Player conceded.
    Concede,
    /// Max steps reached.
    MaxSteps,
    /// Game is still in progress.
    InProgress,
}

/// Summary of a turn.
#[derive(Debug, Clone)]
pub struct TurnSummary {
    /// Turn number.
    pub turn: u32,
    /// Player whose turn it was.
    pub player: PlayerId,
    /// Actions taken this turn.
    pub actions: Vec<ActionSummary>,
    /// Compact game state after the turn.
    pub state_after: String,
}

/// Summary of an action.
#[derive(Debug, Clone)]
pub struct ActionSummary {
    /// The action taken.
    pub action: String,
    /// Was it a prompt response or free action.
    pub prompt_response: bool,
    /// Error if action failed.
    pub error: Option<String>,
}

/// Configuration for running a game.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Maximum number of game steps before stopping.
    pub max_steps: u32,
    /// Whether to record full history.
    pub record_history: bool,
    /// Verbose logging to stderr.
    pub verbose: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            max_steps: 1000,
            record_history: true,
            verbose: false,
        }
    }
}

/// Run a game between two AI controllers.
///
/// Returns the game result after completion or max steps.
pub fn run_game(
    game: &mut GameState,
    p1_ai: &mut dyn AiController,
    p2_ai: &mut dyn AiController,
    config: &RunConfig,
) -> GameResult {
    let mut steps = 0u32;
    let mut turns = 0u32;
    let mut history = Vec::new();
    let mut current_turn_actions = Vec::new();
    let mut last_turn_player = PlayerId::P1;

    while steps < config.max_steps {
        let result = game.step();
        steps += 1;

        match result {
            StepResult::GameOver { winner } => {
                // Record final turn
                if config.record_history && !current_turn_actions.is_empty() {
                    let view = game.view_for_player(last_turn_player);
                    history.push(TurnSummary {
                        turn: turns,
                        player: last_turn_player,
                        actions: current_turn_actions.clone(),
                        state_after: render_game_view_compact(&view),
                    });
                }

                return GameResult {
                    winner: Some(winner),
                    turns,
                    steps,
                    history,
                    p1_prizes_remaining: game.view_for_player(PlayerId::P1).my_prizes_count,
                    p2_prizes_remaining: game.view_for_player(PlayerId::P2).my_prizes_count,
                    end_reason: GameEndReason::PrizesTaken, // Could be any win condition
                };
            }

            StepResult::Prompt { prompt, for_player } => {
                let view = game.view_for_player(for_player);
                let actions = match for_player {
                    PlayerId::P1 => p1_ai.propose_prompt_response(&view, &prompt),
                    PlayerId::P2 => p2_ai.propose_prompt_response(&view, &prompt),
                };

                if config.verbose {
                    eprintln!("[Step {}] {:?} prompt: {:?}", steps, for_player, prompt);
                    eprintln!("  Actions: {:?}", actions);
                }

                // Apply actions
                for action in actions {
                    let action_str = format!("{:?}", action);
                    match game.apply_action(for_player, action) {
                        Ok(_) => {
                            current_turn_actions.push(ActionSummary {
                                action: action_str,
                                prompt_response: true,
                                error: None,
                            });
                        }
                        Err(e) => {
                            if config.verbose {
                                eprintln!("  Error: {:?}", e);
                            }
                            current_turn_actions.push(ActionSummary {
                                action: action_str,
                                prompt_response: true,
                                error: Some(format!("{:?}", e)),
                            });
                        }
                    }
                }
            }

            StepResult::Continue => {
                // Check if we're in a phase where AI can take free actions
                let current_player = game.turn.player;

                // Record turn transition
                if current_player != last_turn_player {
                    if config.record_history && !current_turn_actions.is_empty() {
                        let view = game.view_for_player(last_turn_player);
                        history.push(TurnSummary {
                            turn: turns,
                            player: last_turn_player,
                            actions: current_turn_actions.clone(),
                            state_after: render_game_view_compact(&view),
                        });
                    }
                    current_turn_actions.clear();
                    last_turn_player = current_player;
                    turns += 1;
                }

                let view = game.view_for_player(current_player);

                // Only propose free actions during Main/Attack phases
                if matches!(view.phase, tcg_rules_ex::Phase::Main | tcg_rules_ex::Phase::Attack) {
                    let actions = match current_player {
                        PlayerId::P1 => p1_ai.propose_free_actions(&view),
                        PlayerId::P2 => p2_ai.propose_free_actions(&view),
                    };

                    if config.verbose && !actions.is_empty() {
                        eprintln!("[Step {}] {:?} free actions: {:?}", steps, current_player, actions);
                    }

                    // Apply first action (AIs typically return one at a time)
                    if let Some(action) = actions.into_iter().next() {
                        let action_str = format!("{:?}", action);
                        match game.apply_action(current_player, action) {
                            Ok(_) => {
                                current_turn_actions.push(ActionSummary {
                                    action: action_str,
                                    prompt_response: false,
                                    error: None,
                                });
                            }
                            Err(e) => {
                                if config.verbose {
                                    eprintln!("  Error: {:?}", e);
                                }
                                current_turn_actions.push(ActionSummary {
                                    action: action_str,
                                    prompt_response: false,
                                    error: Some(format!("{:?}", e)),
                                });
                            }
                        }
                    }
                }
            }

            StepResult::Event { event } => {
                if config.verbose {
                    eprintln!("[Step {}] Event: {:?}", steps, event);
                }
            }
        }
    }

    // Max steps reached
    GameResult {
        winner: None,
        turns,
        steps,
        history,
        p1_prizes_remaining: game.view_for_player(PlayerId::P1).my_prizes_count,
        p2_prizes_remaining: game.view_for_player(PlayerId::P2).my_prizes_count,
        end_reason: GameEndReason::MaxSteps,
    }
}

/// Convenience function to run ReactAi (P1) vs RandomAiV4 (P2).
pub fn run_react_vs_v4(
    game: &mut GameState,
    react_ai: &mut ReactAi,
    v4_seed: u64,
    config: &RunConfig,
) -> GameResult {
    let mut v4_ai = RandomAiV4::new(v4_seed);
    run_game(game, react_ai, &mut v4_ai, config)
}

/// Observation for a single step, suitable for returning to Python.
#[derive(Debug, Clone)]
pub struct StepObservation {
    /// Rendered game state.
    pub game_state: String,
    /// Compact summary.
    pub compact: String,
    /// Current phase.
    pub phase: String,
    /// Current player.
    pub current_player: String,
    /// Whether there's a pending prompt.
    pub has_prompt: bool,
    /// The prompt if any.
    pub prompt_description: Option<String>,
    /// Available actions hint.
    pub available_actions: Vec<String>,
    /// Whether game is over.
    pub game_over: bool,
    /// Winner if game over.
    pub winner: Option<String>,
    /// Prizes remaining for each player.
    pub prizes: (usize, usize),
}

/// Create an observation from a game state for a specific player.
pub fn create_observation(game: &GameState, for_player: PlayerId) -> StepObservation {
    use super::render::render_game_view;

    let view = game.view_for_player(for_player);
    let hints = &view.action_hints;

    let mut available_actions = Vec::new();

    if !hints.playable_basic_ids.is_empty() {
        available_actions.push(format!("PlayBasic ({})", hints.playable_basic_ids.len()));
    }
    if !hints.playable_energy_ids.is_empty() {
        available_actions.push(format!("AttachEnergy ({})", hints.playable_energy_ids.len()));
    }
    if !hints.playable_evolution_ids.is_empty() {
        available_actions.push(format!("Evolve ({})", hints.playable_evolution_ids.len()));
    }
    if !hints.playable_trainer_ids.is_empty() {
        available_actions.push(format!("PlayTrainer ({})", hints.playable_trainer_ids.len()));
    }
    if hints.can_declare_attack {
        available_actions.push(format!("Attack ({})", hints.usable_attacks.len()));
    }
    if hints.can_end_turn {
        available_actions.push("EndTurn".to_string());
    }

    let prompt_desc = view.pending_prompt.as_ref().map(|p| format!("{:?}", p));

    StepObservation {
        game_state: render_game_view(&view),
        compact: render_game_view_compact(&view),
        phase: format!("{:?}", view.phase),
        current_player: format!("{:?}", view.current_player),
        has_prompt: view.pending_prompt.is_some(),
        prompt_description: prompt_desc,
        available_actions,
        game_over: false, // Caller should check StepResult
        winner: None,
        prizes: (view.my_prizes_count, view.opponent_prizes_count),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_config_default() {
        let config = RunConfig::default();
        assert_eq!(config.max_steps, 1000);
        assert!(config.record_history);
    }
}
