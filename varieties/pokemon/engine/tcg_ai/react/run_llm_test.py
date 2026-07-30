#!/usr/bin/env python3
"""
Test script for React AI LLM integration.

This script tests the LLM's ability to understand Pokemon TCG game states
and generate valid actions, without needing the full Rust game engine.

Usage:
    python run_llm_test.py --api-key <key> --model <model> [--num-games N]
"""

import asyncio
import json
import os
import sys
import time
from dataclasses import dataclass
from typing import Any

# Add parent directory to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from react.prompts import SYSTEM_PROMPT, build_user_prompt
from react.react_agent import ReactAgent, ReactAgentConfig, ParsedAction


# Sample game states for testing
SAMPLE_GAME_STATES = [
    # Game state 1: Setup phase - choose starting active
    {
        "name": "Setup: Choose Starting Active",
        "state": """=== POKEMON TCG GAME STATE ===
Phase: Setup | Current Turn: P1 | You are: P1

>>> PENDING ACTION REQUIRED <<<
Choose your starting Active Pokemon from: [101, 102, 103]

=== YOUR SIDE ===
Active: None

Bench:
  (empty)

Hand:
  - CG-45 Bulbasaur (id:101) [Basic, Grass, HP:50]
  - CG-67 Treecko (id:102) [Basic, Grass, HP:40]
  - CG-60 Seedot (id:103) [Basic, Grass, HP:40]
  - CG-34 Ivysaur (id:104) [Stage1, Grass, HP:80]
  - ENERGY-GRASS (id:105)
  - ENERGY-GRASS (id:106)
  - CG-87 Potion (id:107)

Deck: 53 cards | Prizes: 6 remaining

=== OPPONENT SIDE ===
Active: None
Bench:
  (empty)
Hand: 7 cards | Deck: 53 cards | Prizes: 6 remaining

=== AVAILABLE ACTIONS ===
  (waiting for prompt response: choose starting active)""",
        "expected_action": "ChooseActive",
        "valid_card_ids": [101, 102, 103],
    },

    # Game state 2: Main phase - play basic, attach energy, attack
    {
        "name": "Main Phase: Multiple Options",
        "state": """=== POKEMON TCG GAME STATE ===
Phase: Main | Current Turn: P1 | You are: P1

=== YOUR SIDE ===
Active: Bulbasaur (Grass)
  ID: 101
  HP: 50/50
  Energy: ENERGY-GRASS (id:105)
  Attacks:
    - Razor Leaf: 20 damage, Cost: [Grass]

Bench:
  Bench 1: Treecko (Grass)
    ID: 102
    HP: 40/40
    Energy: None

Hand:
  - CG-60 Seedot (id:103) [Basic, Grass, HP:40]
  - ENERGY-GRASS (id:106)
  - CG-34 Ivysaur (id:107) [Stage1, Grass, HP:80, evolves from Bulbasaur]
  - CG-87 Potion (id:108)

Deck: 50 cards | Prizes: 6 remaining

=== OPPONENT SIDE ===
Active: Squirtle (Water)
  ID: 201
  HP: 40/50
  Energy: ENERGY-WATER (id:205)
  Weakness: Lightning x2

Bench:
  (empty)
Hand: 5 cards | Deck: 51 cards | Prizes: 6 remaining

=== AVAILABLE ACTIONS ===
  Play Basic Pokemon: CG-60 Seedot (id:103)
  Attach Energy: ENERGY-GRASS (id:106) -> targets available
  Evolve: CG-34 Ivysaur (id:107) -> targets: [101]
  Play Trainer: CG-87 Potion (id:108)
  Attacks available:
    - Razor Leaf: 20 damage, Cost: 1 energy
  End Turn: Available""",
        "expected_actions": ["EvolveFromHand", "AttachEnergy", "PlayBasic", "DeclareAttack", "PlayTrainer", "EndTurn"],
    },

    # Game state 3: Opponent's active is low HP - should attack for KO
    {
        "name": "Attack for Knockout",
        "state": """=== POKEMON TCG GAME STATE ===
Phase: Main | Current Turn: P1 | You are: P1

=== YOUR SIDE ===
Active: Venusaur (Grass) [EX]
  ID: 101
  HP: 120/150
  Energy: ENERGY-GRASS (id:105), ENERGY-GRASS (id:106), ENERGY-GRASS (id:107)
  Attacks:
    - Green Blast: 50+ damage, Cost: [Grass, Grass, Colorless] (does 10 more for each Grass energy)

Bench:
  Bench 1: Grovyle (Grass)
    ID: 102
    HP: 80/80
    Energy: ENERGY-GRASS (id:108)

Hand:
  - CG-87 Potion (id:110)
  - ENERGY-GRASS (id:111)

Deck: 40 cards | Prizes: 4 remaining

=== OPPONENT SIDE ===
Active: Wartortle (Water)
  ID: 201
  HP: 20/70
  Energy: ENERGY-WATER (id:205), ENERGY-WATER (id:206)
  Weakness: Lightning x2

Bench:
  Bench 1: Blastoise (Water)
    ID: 202
    HP: 100/100
    Energy: ENERGY-WATER (id:207)
Hand: 3 cards | Deck: 35 cards | Prizes: 5 remaining

=== AVAILABLE ACTIONS ===
  Attach Energy: ENERGY-GRASS (id:111) -> targets available
  Play Trainer: CG-87 Potion (id:110)
  Attacks available:
    - Green Blast: 80 damage (50 + 30 bonus from 3 Grass energy), Cost: 3 energy
  End Turn: Available""",
        "expected_action": "DeclareAttack",
        "reasoning": "Attack does 80 damage, opponent has 20 HP - guaranteed knockout",
    },

    # Game state 4: Choose new active after KO
    {
        "name": "Choose New Active After KO",
        "state": """=== POKEMON TCG GAME STATE ===
Phase: Main | Current Turn: P1 | You are: P1

>>> PENDING ACTION REQUIRED <<<
Choose a new Active Pokemon from bench: [102, 103]

=== YOUR SIDE ===
Active: None (Knocked Out!)

Bench:
  Bench 1: Grovyle (Grass)
    ID: 102
    HP: 80/80
    Energy: ENERGY-GRASS (id:108)
  Bench 2: Treecko (Grass)
    ID: 103
    HP: 40/40
    Energy: None

Hand:
  - ENERGY-GRASS (id:111)

Deck: 38 cards | Prizes: 3 remaining

=== OPPONENT SIDE ===
Active: Blastoise (Water)
  ID: 202
  HP: 90/100
  Energy: ENERGY-WATER (id:207), ENERGY-WATER (id:208)

Bench:
  (empty)
Hand: 4 cards | Deck: 32 cards | Prizes: 4 remaining

=== AVAILABLE ACTIONS ===
  (waiting for prompt response: choose new active)""",
        "expected_action": "ChooseNewActive",
        "valid_card_ids": [102, 103],
        "reasoning": "Grovyle (102) has more HP and energy - better choice",
    },
]


async def test_llm_response(
    agent: ReactAgent,
    game_state: dict,
    verbose: bool = True,
) -> dict:
    """Test LLM response for a single game state."""
    from react.headless_game import GameObservation, Phase, ActionHints

    # Create a mock observation
    observation = GameObservation(
        player_id="P1",
        phase=Phase.MAIN,
        current_player="P1",
        my_hand=[],
        my_deck_count=50,
        my_discard=[],
        my_prizes_count=6,
        my_active=None,
        my_bench=[],
        opponent_hand_count=5,
        opponent_deck_count=50,
        opponent_prizes_count=6,
        opponent_active=None,
        opponent_bench=[],
        stadium_in_play=None,
        action_hints=ActionHints(),
    )

    # Override render to use our sample state
    class MockObservation:
        def render(self):
            return game_state["state"]

        def render_compact(self):
            return game_state["name"]

        @property
        def action_hints(self):
            return ActionHints()

    mock_obs = MockObservation()

    start_time = time.time()

    try:
        action = await agent.select_action(mock_obs)
        duration = time.time() - start_time

        result = {
            "name": game_state["name"],
            "success": True,
            "action_type": action.action_type,
            "action": action.to_action_dict(),
            "reason": action.reason,
            "duration": duration,
            "raw_response": action.raw_response[:500] if action.raw_response else None,
        }

        # Check if action matches expected
        if "expected_action" in game_state:
            result["expected"] = game_state["expected_action"]
            result["matched"] = action.action_type == game_state["expected_action"]
        elif "expected_actions" in game_state:
            result["expected"] = game_state["expected_actions"]
            result["matched"] = action.action_type in game_state["expected_actions"]

        if verbose:
            status = "OK" if result.get("matched", True) else "MISMATCH"
            print(f"  [{status}] {action.action_type}")
            if action.reason:
                print(f"       Reason: {action.reason[:100]}")

    except Exception as e:
        duration = time.time() - start_time
        result = {
            "name": game_state["name"],
            "success": False,
            "error": str(e),
            "duration": duration,
        }
        if verbose:
            print(f"  [ERROR] {e}")

    return result


async def run_model_test(
    api_key: str,
    model: str,
    num_rounds: int = 1,
    verbose: bool = True,
) -> dict:
    """Run tests for a specific model."""
    print(f"\n{'='*60}")
    print(f"Model: {model}")
    print(f"{'='*60}")

    agent = ReactAgent(
        api_key=api_key,
        config=ReactAgentConfig(
            model=model,
            temperature=0.3,
            max_tokens=512,
        ),
    )

    results = []

    for round_num in range(num_rounds):
        if num_rounds > 1:
            print(f"\n--- Round {round_num + 1} ---")

        for game_state in SAMPLE_GAME_STATES:
            print(f"\nTest: {game_state['name']}")
            result = await test_llm_response(agent, game_state, verbose)
            result["round"] = round_num
            results.append(result)

    # Summary
    successes = sum(1 for r in results if r["success"])
    matches = sum(1 for r in results if r.get("matched", False))
    total = len(results)

    avg_duration = sum(r["duration"] for r in results) / total if results else 0

    summary = {
        "model": model,
        "total_tests": total,
        "successes": successes,
        "matches": matches,
        "success_rate": successes / total if total > 0 else 0,
        "match_rate": matches / total if total > 0 else 0,
        "avg_duration": avg_duration,
        "results": results,
    }

    print(f"\n--- {model} Summary ---")
    print(f"Tests: {total}, Successes: {successes}, Matches: {matches}")
    print(f"Success Rate: {summary['success_rate']*100:.1f}%")
    print(f"Match Rate: {summary['match_rate']*100:.1f}%")
    print(f"Avg Duration: {avg_duration:.2f}s")

    return summary


async def main():
    import argparse

    parser = argparse.ArgumentParser(description="Test React AI LLM integration")
    parser.add_argument("--api-key", required=True, help="OpenAI API key")
    parser.add_argument("--model", default="gpt-4.1-mini", help="Model to test")
    parser.add_argument("--models", nargs="+", help="Multiple models to test")
    parser.add_argument("--num-rounds", type=int, default=1, help="Number of rounds per model")
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")

    args = parser.parse_args()

    models = args.models if args.models else [args.model]

    all_results = {}

    for model in models:
        result = await run_model_test(
            api_key=args.api_key,
            model=model,
            num_rounds=args.num_rounds,
            verbose=args.verbose or len(models) == 1,
        )
        all_results[model] = result

    # Final comparison if multiple models
    if len(models) > 1:
        print(f"\n{'='*60}")
        print("FINAL COMPARISON")
        print(f"{'='*60}")
        print(f"{'Model':<20} {'Success%':>10} {'Match%':>10} {'Avg Time':>10}")
        print("-" * 50)
        for model, result in all_results.items():
            print(f"{model:<20} {result['success_rate']*100:>9.1f}% {result['match_rate']*100:>9.1f}% {result['avg_duration']:>9.2f}s")


if __name__ == "__main__":
    asyncio.run(main())
