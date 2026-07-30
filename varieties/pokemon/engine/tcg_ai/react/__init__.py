"""
ReAct Agent for Pokemon TCG.

This module provides a ReAct-style AI agent for playing Pokemon TCG
against the deterministic AI (v4) opponent.

Architecture Overview:
---------------------
1. HeadlessGame: Manages game state and provides text-based observations
2. ReactAgent: LLM-based agent that selects actions from observations
3. GameRunner: Orchestrates gameplay between ReactAgent and AI opponent

Example Usage:
--------------
```python
import asyncio
from tcg_ai.react import HeadlessGame, ReactAgent, GameRunner

async def main():
    # Create a headless game
    game = HeadlessGame(seed=42, p1_deck=deck1, p2_deck=deck2)

    # Create a react agent with OpenAI
    agent = ReactAgent(
        api_key="your-api-key",
        model="gpt-4",
    )

    # Run a game
    runner = GameRunner(game, agent)
    result = await runner.run()

    print(f"Winner: {result['winner']}")
    print(f"Turns: {result['turns']}")

asyncio.run(main())
```
"""

from .headless_game import HeadlessGame, GameObservation
from .react_agent import ReactAgent, ReactAgentConfig
from .game_runner import GameRunner, GameResult
from .prompts import SYSTEM_PROMPT, build_user_prompt

__all__ = [
    "HeadlessGame",
    "GameObservation",
    "ReactAgent",
    "ReactAgentConfig",
    "GameRunner",
    "GameResult",
    "SYSTEM_PROMPT",
    "build_user_prompt",
]
