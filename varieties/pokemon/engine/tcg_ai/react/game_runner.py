"""
Game runner for ReAct agent vs AI opponent matches.

This module provides utilities for running complete games between
the ReAct agent and deterministic AI opponents.
"""

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Any, Optional, List, Dict
from enum import Enum

from .headless_game import HeadlessGame, GameObservation
from .react_agent import ReactAgent, ParsedAction

logger = logging.getLogger(__name__)


class GameEndReason(str, Enum):
    """How the game ended."""
    PRIZES_TAKEN = "PrizesTaken"
    NO_POKEMON = "NoPokemon"
    DECK_OUT = "DeckOut"
    CONCEDE = "Concede"
    MAX_STEPS = "MaxSteps"
    ERROR = "Error"


@dataclass
class TurnSummary:
    """Summary of a turn."""
    turn: int
    player: str
    actions: List[dict]
    state_after: str


@dataclass
class GameResult:
    """Result of a completed game."""
    winner: Optional[str]
    turns: int
    steps: int
    end_reason: GameEndReason
    p1_prizes_remaining: int
    p2_prizes_remaining: int
    history: List[TurnSummary] = field(default_factory=list)
    agent_history: List[dict] = field(default_factory=list)
    error: Optional[str] = None


@dataclass
class RunnerConfig:
    """Configuration for the game runner."""
    max_steps: int = 1000
    record_history: bool = True
    verbose: bool = False
    step_delay: float = 0.0  # Delay between steps (for debugging)


class GameRunner:
    """Runs games between a ReAct agent and AI opponent.

    This class orchestrates the game loop, handling:
    - Agent action selection
    - AI opponent turns
    - Game state updates
    - Result tracking
    """

    def __init__(
        self,
        game: HeadlessGame,
        agent: ReactAgent,
        config: Optional[RunnerConfig] = None,
    ):
        """Initialize the game runner.

        Args:
            game: The headless game environment.
            agent: The ReAct agent (player 1).
            config: Runner configuration.
        """
        self.game = game
        self.agent = agent
        self.config = config or RunnerConfig()

        # State tracking
        self._step_count = 0
        self._turn_count = 0
        self._history: List[TurnSummary] = []
        self._current_turn_actions: List[dict] = []
        self._last_action_result: Optional[str] = None

    async def run(self) -> GameResult:
        """Run a complete game.

        Returns:
            Game result with statistics and history.
        """
        try:
            # Initialize game
            observation = await self.game.initialize()
            self._step_count = 0
            self._turn_count = 0
            self._history.clear()
            self._current_turn_actions.clear()

            if self.config.verbose:
                logger.info("Game initialized")
                logger.info(observation.render_compact())

            # Main game loop
            while self._step_count < self.config.max_steps:
                # Check for game over
                if observation.terminated:
                    return self._build_result(observation, GameEndReason.PRIZES_TAKEN)

                # Get action from agent or AI
                if observation.current_player == "P1":
                    action = await self._agent_turn(observation)
                else:
                    # AI opponent's turn - game handles this internally
                    await self.game.run_ai_opponent_turn()
                    observation, _, terminated, _ = await self.game.step({"action": "Continue"})
                    self._step_count += 1
                    continue

                # Execute action
                observation, reward, terminated, info = await self.game.step(action.to_action_dict())
                self._step_count += 1

                # Record action
                self._current_turn_actions.append({
                    "action": action.to_action_dict(),
                    "reason": action.reason,
                    "result": self._last_action_result,
                })

                # Track turn changes
                if observation.current_player != "P1":
                    self._record_turn_end("P1", observation)

                if self.config.verbose:
                    logger.info(f"Step {self._step_count}: {action.action_type}")
                    logger.info(observation.render_compact())

                if self.config.step_delay > 0:
                    await asyncio.sleep(self.config.step_delay)

                if terminated:
                    return self._build_result(observation, GameEndReason.PRIZES_TAKEN)

            # Max steps reached
            return self._build_result(observation, GameEndReason.MAX_STEPS)

        except Exception as e:
            logger.error(f"Game error: {e}")
            return GameResult(
                winner=None,
                turns=self._turn_count,
                steps=self._step_count,
                end_reason=GameEndReason.ERROR,
                p1_prizes_remaining=6,
                p2_prizes_remaining=6,
                error=str(e),
            )

    async def _agent_turn(self, observation: GameObservation) -> ParsedAction:
        """Handle the agent's turn.

        Args:
            observation: Current game observation.

        Returns:
            Parsed action from the agent.
        """
        action = await self.agent.select_action(
            observation,
            last_result=self._last_action_result,
        )

        if self.config.verbose:
            logger.info(f"Agent selected: {action.action_type}")
            if action.reason:
                logger.info(f"  Reason: {action.reason}")

        return action

    def _record_turn_end(self, player: str, observation: GameObservation) -> None:
        """Record the end of a turn.

        Args:
            player: Player whose turn ended.
            observation: State after the turn.
        """
        if self.config.record_history and self._current_turn_actions:
            self._history.append(TurnSummary(
                turn=self._turn_count,
                player=player,
                actions=self._current_turn_actions.copy(),
                state_after=observation.render_compact(),
            ))
        self._current_turn_actions.clear()
        self._turn_count += 1

    def _build_result(
        self,
        observation: GameObservation,
        end_reason: GameEndReason,
    ) -> GameResult:
        """Build the game result.

        Args:
            observation: Final observation.
            end_reason: How the game ended.

        Returns:
            Game result.
        """
        return GameResult(
            winner=observation.winner,
            turns=self._turn_count,
            steps=self._step_count,
            end_reason=end_reason,
            p1_prizes_remaining=observation.my_prizes_count,
            p2_prizes_remaining=observation.opponent_prizes_count,
            history=self._history.copy() if self.config.record_history else [],
            agent_history=self.agent.get_history(),
        )


async def run_single_game(
    p1_deck: List[str],
    p2_deck: List[str],
    api_key: str,
    seed: int = 42,
    model: str = "gpt-4",
    ai_version: int = 4,
    max_steps: int = 1000,
    verbose: bool = False,
) -> GameResult:
    """Convenience function to run a single game.

    Args:
        p1_deck: Player 1's deck (ReAct agent).
        p2_deck: Player 2's deck (AI opponent).
        api_key: API key for LLM.
        seed: Random seed.
        model: LLM model to use.
        ai_version: AI opponent version (1-4).
        max_steps: Maximum game steps.
        verbose: Enable verbose logging.

    Returns:
        Game result.
    """
    from .react_agent import ReactAgentConfig

    game = HeadlessGame(
        seed=seed,
        p1_deck=p1_deck,
        p2_deck=p2_deck,
        ai_opponent_version=ai_version,
    )

    agent = ReactAgent(
        api_key=api_key,
        config=ReactAgentConfig(model=model),
    )

    runner = GameRunner(
        game=game,
        agent=agent,
        config=RunnerConfig(
            max_steps=max_steps,
            record_history=True,
            verbose=verbose,
        ),
    )

    return await runner.run()


async def run_evaluation(
    p1_deck: List[str],
    p2_deck: List[str],
    api_key: str,
    num_games: int = 10,
    model: str = "gpt-4",
    ai_version: int = 4,
    base_seed: int = 42,
    max_steps: int = 1000,
    verbose: bool = False,
) -> Dict[str, Any]:
    """Run multiple games for evaluation.

    Args:
        p1_deck: Player 1's deck.
        p2_deck: Player 2's deck.
        api_key: API key for LLM.
        num_games: Number of games to run.
        model: LLM model to use.
        ai_version: AI opponent version.
        base_seed: Base random seed.
        max_steps: Maximum steps per game.
        verbose: Enable verbose logging.

    Returns:
        Evaluation metrics dictionary.
    """
    results = []

    for i in range(num_games):
        seed = base_seed + i
        if verbose:
            logger.info(f"Running game {i + 1}/{num_games} (seed={seed})")

        result = await run_single_game(
            p1_deck=p1_deck,
            p2_deck=p2_deck,
            api_key=api_key,
            seed=seed,
            model=model,
            ai_version=ai_version,
            max_steps=max_steps,
            verbose=verbose,
        )
        results.append(result)

    # Calculate metrics
    wins = sum(1 for r in results if r.winner == "P1")
    losses = sum(1 for r in results if r.winner == "P2")
    draws = sum(1 for r in results if r.winner is None)

    avg_turns = sum(r.turns for r in results) / len(results) if results else 0
    avg_steps = sum(r.steps for r in results) / len(results) if results else 0

    return {
        "total_games": num_games,
        "wins": wins,
        "losses": losses,
        "draws": draws,
        "win_rate": wins / num_games if num_games > 0 else 0,
        "avg_turns": avg_turns,
        "avg_steps": avg_steps,
        "results": results,
    }


# Example usage script
if __name__ == "__main__":
    import os

    async def main():
        # Example decks (would be actual card lists in production)
        deck1 = ["CG-001"] * 4 + ["ENERGY-GRASS"] * 20 + ["CG-002"] * 36
        deck2 = ["CG-003"] * 4 + ["ENERGY-FIRE"] * 20 + ["CG-004"] * 36

        api_key = os.environ.get("OPENAI_API_KEY", "")
        if not api_key:
            print("Set OPENAI_API_KEY environment variable")
            return

        result = await run_single_game(
            p1_deck=deck1,
            p2_deck=deck2,
            api_key=api_key,
            seed=42,
            model="gpt-4",
            verbose=True,
        )

        print(f"\nGame Over!")
        print(f"Winner: {result.winner}")
        print(f"Turns: {result.turns}")
        print(f"Steps: {result.steps}")
        print(f"End Reason: {result.end_reason}")

    asyncio.run(main())
