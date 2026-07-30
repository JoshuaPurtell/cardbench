//! ReAct AI implementation for Pokemon TCG.
//!
//! This module provides a ReAct-style AI agent that uses an LLM to make decisions.
//! It implements the `AiController` trait to be compatible with the game engine.

use std::sync::{Arc, Mutex};
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use tcg_core::{Action, GameView, Prompt};
use crate::traits::AiController;
use super::render::render_game_view;
use super::action_parser::{parse_response, ParsedResponse};

/// Configuration for the ReAct AI.
#[derive(Debug, Clone)]
pub struct ReactAiConfig {
    /// System prompt for the LLM.
    pub system_prompt: String,
    /// Temperature for LLM sampling.
    pub temperature: f32,
    /// Maximum tokens for LLM response.
    pub max_tokens: u32,
    /// Whether to include reasoning in prompts.
    pub include_reasoning: bool,
    /// Strict to random action on parse failure.
    pub strict_to_random: bool,
}

impl Default for ReactAiConfig {
    fn default() -> Self {
        Self {
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            temperature: 0.3,
            max_tokens: 256,
            include_reasoning: true,
            strict_to_random: true,
        }
    }
}

/// Default system prompt for the ReAct AI.
pub const DEFAULT_SYSTEM_PROMPT: &str = r#"You are an expert Pokemon TCG player. Analyze the game state and respond with a JSON action.

## CRITICAL: Two Types of Situations

### 1. PENDING PROMPT (when game asks you a question)
Look for "=== PENDING PROMPT ===" in the game state. You MUST respond with the matching action type:

| Prompt Type | Required Action | Fields |
|-------------|-----------------|--------|
| ChooseStartingActive | ChooseActive | card_id |
| ChooseBenchBasics | ChooseBench | card_ids (array) |
| ChooseCardsFromDeck | TakeCardsFromDeck | card_ids (array) |
| ChooseCardsFromDiscard | TakeCardsFromDiscard | card_ids (array) |
| ChooseCardsFromHand | DiscardCardsFromHand or ReturnCardsFromHandToDeck | card_ids (array) |
| ChoosePokemonInPlay | ChoosePokemonTargets | target_ids (array) |
| ChooseAttachedEnergy | ChooseAttachedEnergy | energy_ids (array) |
| ChooseNewActive | ChooseNewActive | card_id |
| ChoosePrizeCards | ChoosePrizeCards | card_ids (array) |
| ChooseAttack | DeclareAttack | attack (name string) |
| ReorderDeckTop | ReorderDeckTop | card_ids (array in desired order) |

IMPORTANT: When you see a prompt, use ONLY the matching action type above. Do NOT use PlayBasic, AttachEnergy, etc. when responding to prompts!

### 2. FREE ACTIONS (during Main/Attack phase, no pending prompt)
When there is NO pending prompt and it's your turn:

| Action | Fields | When to use |
|--------|--------|-------------|
| PlayBasic | card_id | Play a Basic Pokemon from hand to bench |
| AttachEnergy | energy_id, target_id | Attach 1 energy per turn to your Pokemon |
| EvolveFromHand | card_id, target_id | Evolve a Pokemon (not first turn played) |
| PlayTrainer | card_id | Play a Trainer card |
| DeclareAttack | attack (name) | Attack with your active Pokemon |
| Retreat | to_bench_id | Switch active with benched (pay retreat cost) |
| EndTurn | (none) | End your turn |

## JSON Response Format

Always respond with ONLY valid JSON:
```json
{"action": "ActionType", "card_id": 123, "reason": "brief explanation"}
```

For array fields:
```json
{"action": "TakeCardsFromDeck", "card_ids": [45, 67], "reason": "searching for evolution"}
```

For attacks:
```json
{"action": "DeclareAttack", "attack": "Razor Leaf", "reason": "enough energy to attack"}
```

## Reading the Game State

- Card IDs are shown as (id:XX) - use the number XX
- "Options:" lists valid choices for prompts - pick from these IDs only
- "Usable attacks:" shows attacks you can currently use
- Energy requirements like [G][C] mean 1 Grass + 1 any color

## Strategy

1. Build energy on attackers before attacking
2. Evolve Pokemon for better HP and attacks
3. Keep backup attackers on bench
4. Type advantage: Weakness = 2x damage received
5. When searching deck: get evolution cards or energy you need
6. Prioritize knocking out opponent Pokemon for prizes

Respond with a single JSON object only. No explanation outside the JSON."#;

/// Trait for LLM providers.
pub trait LlmProvider: Send + Sync {
    /// Generate a response from the LLM.
    fn generate(&self, system: &str, user: &str, config: &ReactAiConfig) -> Result<String, String>;
}

/// A placeholder LLM provider that returns instructions for manual implementation.
/// Replace this with actual API calls to OpenAI, Anthropic, etc.
#[derive(Clone)]
pub struct PlaceholderLlmProvider;

impl LlmProvider for PlaceholderLlmProvider {
    fn generate(&self, _system: &str, _user: &str, _config: &ReactAiConfig) -> Result<String, String> {
        Err("LLM provider not implemented. Implement LlmProvider trait with your API.".to_string())
    }
}

/// Synchronous LLM provider wrapper for async APIs.
/// This allows wrapping async LLM calls for use in the sync AiController interface.
pub struct SyncLlmWrapper<F>
where
    F: Fn(&str, &str, &ReactAiConfig) -> Result<String, String> + Send + Sync,
{
    pub call_fn: F,
}

impl<F> LlmProvider for SyncLlmWrapper<F>
where
    F: Fn(&str, &str, &ReactAiConfig) -> Result<String, String> + Send + Sync,
{
    fn generate(&self, system: &str, user: &str, config: &ReactAiConfig) -> Result<String, String> {
        (self.call_fn)(system, user, config)
    }
}

/// ReAct AI agent for Pokemon TCG.
pub struct ReactAi {
    /// Configuration for the AI.
    config: ReactAiConfig,
    /// LLM provider for generating responses.
    llm: Arc<dyn LlmProvider>,
    /// Random number generator for strict.
    rng: Mutex<ChaCha8Rng>,
    /// History of actions taken (for debugging/analysis).
    history: Mutex<Vec<ActionHistoryEntry>>,
    /// TODO list maintained across turns.
    todo_list: Mutex<Vec<String>>,
}

/// Entry in the action history.
#[derive(Debug, Clone)]
pub struct ActionHistoryEntry {
    pub game_state_summary: String,
    pub llm_response: Option<String>,
    pub parsed_action: Option<Action>,
    pub error: Option<String>,
}

impl ReactAi {
    /// Create a new ReactAi with default configuration.
    pub fn new(seed: u64) -> Self {
        Self::with_config(seed, ReactAiConfig::default(), Arc::new(PlaceholderLlmProvider))
    }

    /// Create a new ReactAi with custom configuration and LLM provider.
    pub fn with_config(seed: u64, config: ReactAiConfig, llm: Arc<dyn LlmProvider>) -> Self {
        Self {
            config,
            llm,
            rng: Mutex::new(ChaCha8Rng::seed_from_u64(seed)),
            history: Mutex::new(Vec::new()),
            todo_list: Mutex::new(vec![
                "Assess board state".to_string(),
                "Build up attackers".to_string(),
                "Take prize cards".to_string(),
            ]),
        }
    }

    /// Create a new ReactAi with a custom LLM call function.
    pub fn with_llm_fn<F>(seed: u64, call_fn: F) -> Self
    where
        F: Fn(&str, &str, &ReactAiConfig) -> Result<String, String> + Send + Sync + 'static,
    {
        Self::with_config(
            seed,
            ReactAiConfig::default(),
            Arc::new(SyncLlmWrapper { call_fn }),
        )
    }

    /// Get the action history.
    pub fn get_history(&self) -> Vec<ActionHistoryEntry> {
        self.history.lock().unwrap().clone()
    }

    /// Get the current TODO list.
    pub fn get_todo_list(&self) -> Vec<String> {
        self.todo_list.lock().unwrap().clone()
    }

    /// Build the user prompt from the game view.
    fn build_user_prompt(&self, view: &GameView) -> String {
        let mut parts = Vec::new();

        // Render game state
        parts.push(render_game_view(view));

        // Add TODO list
        let todos = self.todo_list.lock().unwrap();
        if !todos.is_empty() {
            parts.push(String::new());
            parts.push("=== YOUR TODO LIST ===".to_string());
            for (i, todo) in todos.iter().enumerate() {
                parts.push(format!("{}. {}", i + 1, todo));
            }
        }

        parts.push(String::new());
        parts.push("Choose your action and respond with JSON.".to_string());

        parts.join("\n")
    }

    /// Query the LLM for an action.
    fn query_llm(&self, view: &GameView) -> Result<ParsedResponse, String> {
        let user_prompt = self.build_user_prompt(view);

        let response = self.llm.generate(&self.config.system_prompt, &user_prompt, &self.config)?;

        parse_response(&response, view)
            .map_err(|e| format!("Parse error: {}", e))
    }

    /// Add an entry to the action history.
    fn log_action(&self, summary: String, response: Option<String>, action: Option<Action>, error: Option<String>) {
        let entry = ActionHistoryEntry {
            game_state_summary: summary,
            llm_response: response,
            parsed_action: action,
            error,
        };
        self.history.lock().unwrap().push(entry);
    }

    /// Update TODO list with new items.
    fn update_todos(&self, new_todos: Vec<String>) {
        if !new_todos.is_empty() {
            let mut todos = self.todo_list.lock().unwrap();
            for todo in new_todos {
                if !todos.contains(&todo) {
                    todos.push(todo);
                }
            }
            // Keep list manageable
            while todos.len() > 10 {
                todos.remove(0);
            }
        }
    }

    /// Generate a random strict action.
    fn random_strict_action(&self, view: &GameView) -> Option<Action> {
        let hints = &view.action_hints;
        let mut rng = self.rng.lock().unwrap();

        // Build list of possible actions
        let mut actions = Vec::new();

        // Play basics
        for id in &hints.playable_basic_ids {
            actions.push(Action::PlayBasic { card_id: *id });
        }

        // Attach energy
        for energy_id in &hints.playable_energy_ids {
            for target_id in &hints.attach_targets {
                actions.push(Action::AttachEnergy {
                    energy_id: *energy_id,
                    target_id: *target_id,
                });
            }
        }

        // Evolutions
        for (evo_id, targets) in &hints.evolve_targets_by_card_id {
            for target_id in targets {
                actions.push(Action::EvolveFromHand {
                    card_id: *evo_id,
                    target_id: *target_id,
                });
            }
        }

        // Trainers
        for id in &hints.playable_trainer_ids {
            actions.push(Action::PlayTrainer { card_id: *id });
        }

        // Attack
        if hints.can_declare_attack {
            for attack in &hints.usable_attacks {
                actions.push(Action::DeclareAttack { attack: attack.clone() });
            }
        }

        // End turn
        if hints.can_end_turn {
            actions.push(Action::EndTurn);
        }

        actions.choose(&mut *rng).cloned()
    }

    /// Generate a random prompt response.
    fn random_prompt_response(&self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let mut rng = self.rng.lock().unwrap();

        match prompt {
            Prompt::ChooseStartingActive { options } => {
                if let Some(id) = options.choose(&mut *rng) {
                    vec![Action::ChooseActive { card_id: *id }]
                } else {
                    vec![]
                }
            }

            Prompt::ChooseBenchBasics { options, min, max } => {
                let count = (*min).max(1).min(*max).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(count);
                vec![Action::ChooseBench { card_ids: ids }]
            }

            Prompt::ChooseAttack { attacks, .. } => {
                if let Some(attack) = attacks.choose(&mut *rng) {
                    vec![Action::DeclareAttack { attack: attack.clone() }]
                } else {
                    vec![Action::CancelPrompt]
                }
            }

            Prompt::ChooseCardsFromDeck { options, count, min, max, .. } => {
                let min_val = min.unwrap_or(*count);
                let max_val = max.unwrap_or(*count);
                let target = min_val.max(1).min(max_val).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                vec![Action::TakeCardsFromDeck { card_ids: ids }]
            }

            Prompt::ChooseCardsFromDiscard { options, count, min, max, .. } => {
                let min_val = min.unwrap_or(*count);
                let max_val = max.unwrap_or(*count);
                let target = min_val.max(1).min(max_val).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                vec![Action::TakeCardsFromDiscard { card_ids: ids }]
            }

            Prompt::ChoosePokemonInPlay { options, min, max, .. } => {
                let target = (*min).max(1).min(*max).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                vec![Action::ChoosePokemonTargets { target_ids: ids }]
            }

            Prompt::ReorderDeckTop { options, .. } => {
                let mut ids = options.clone();
                ids.shuffle(&mut *rng);
                vec![Action::ReorderDeckTop { card_ids: ids }]
            }

            Prompt::ChooseAttachedEnergy { count, min, pokemon_id, .. } => {
                // Need to get energy from the pokemon
                let target = min.unwrap_or(*count);
                let pokemon = view.my_active.as_ref()
                    .filter(|p| p.card.id == *pokemon_id)
                    .or_else(|| view.my_bench.iter().find(|p| p.card.id == *pokemon_id));

                if let Some(p) = pokemon {
                    let mut energy_ids: Vec<_> = p.attached_energy.iter().map(|e| e.id).collect();
                    energy_ids.shuffle(&mut *rng);
                    energy_ids.truncate(target);
                    vec![Action::ChooseAttachedEnergy { energy_ids }]
                } else {
                    vec![]
                }
            }

            Prompt::ChooseCardsFromHand { options, count, min, max, return_to_deck, .. } => {
                let min_val = min.unwrap_or(*count);
                let max_val = max.unwrap_or(*count);
                let target = min_val.max(1).min(max_val).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                if *return_to_deck {
                    vec![Action::ReturnCardsFromHandToDeck { card_ids: ids }]
                } else {
                    vec![Action::DiscardCardsFromHand { card_ids: ids }]
                }
            }

            Prompt::ChooseCardsInPlay { options, min, max, .. } => {
                let target = (*min).max(1).min(*max).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                vec![Action::ChooseCardsInPlay { card_ids: ids }]
            }

            Prompt::ChooseDefenderAttack { attacks, .. } => {
                if let Some(attack_name) = attacks.choose(&mut *rng) {
                    vec![Action::ChooseDefenderAttack { attack_name: attack_name.clone() }]
                } else {
                    vec![]
                }
            }

            Prompt::ChoosePokemonAttack { attacks, .. } => {
                if let Some(attack_name) = attacks.choose(&mut *rng) {
                    vec![Action::ChoosePokemonAttack { attack_name: attack_name.clone() }]
                } else {
                    vec![]
                }
            }

            Prompt::ChooseSpecialCondition { options, .. } => {
                if let Some(cond) = options.choose(&mut *rng) {
                    vec![Action::ChooseSpecialCondition { condition: *cond }]
                } else {
                    vec![]
                }
            }

            Prompt::ChoosePrizeCards { options, min, max, .. } => {
                let target = (*min).max(1).min(*max).min(options.len());
                let mut ids: Vec<_> = options.clone();
                ids.shuffle(&mut *rng);
                ids.truncate(target);
                vec![Action::ChoosePrizeCards { card_ids: ids }]
            }

            Prompt::ChooseNewActive { options, .. } => {
                if let Some(id) = options.choose(&mut *rng) {
                    vec![Action::ChooseNewActive { card_id: *id }]
                } else {
                    vec![]
                }
            }
            _ => vec![Action::EndTurn],
        }
    }
}

impl AiController for ReactAi {
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let summary = format!("Prompt: {:?}", prompt);

        // Try LLM first
        match self.query_llm(view) {
            Ok(parsed) => {
                self.log_action(
                    summary,
                    Some(format!("{:?}", parsed.action)),
                    Some(parsed.action.clone()),
                    None,
                );
                self.update_todos(parsed.todo_add);
                vec![parsed.action]
            }
            Err(e) => {
                // Strict to random
                if self.config.strict_to_random {
                    let actions = self.random_prompt_response(view, prompt);
                    self.log_action(
                        summary,
                        None,
                        actions.first().cloned(),
                        Some(format!("LLM failed: {}, using random", e)),
                    );
                    actions
                } else {
                    self.log_action(summary, None, None, Some(e));
                    vec![]
                }
            }
        }
    }

    fn propose_free_actions(&mut self, view: &GameView) -> Vec<Action> {
        let summary = format!("Phase: {:?}", view.phase);

        // Try LLM first
        match self.query_llm(view) {
            Ok(parsed) => {
                self.log_action(
                    summary,
                    Some(format!("{:?}", parsed.action)),
                    Some(parsed.action.clone()),
                    None,
                );
                self.update_todos(parsed.todo_add);
                vec![parsed.action]
            }
            Err(e) => {
                // Strict to random
                if self.config.strict_to_random {
                    if let Some(action) = self.random_strict_action(view) {
                        self.log_action(
                            summary,
                            None,
                            Some(action.clone()),
                            Some(format!("LLM failed: {}, using random", e)),
                        );
                        vec![action]
                    } else {
                        // If no action available, end turn
                        if view.action_hints.can_end_turn {
                            vec![Action::EndTurn]
                        } else {
                            vec![]
                        }
                    }
                } else {
                    self.log_action(summary, None, None, Some(e));
                    vec![]
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_react_ai_creation() {
        let ai = ReactAi::new(42);
        assert!(!ai.get_todo_list().is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = ReactAiConfig::default();
        assert!(config.temperature > 0.0);
        assert!(config.strict_to_random);
    }
}
