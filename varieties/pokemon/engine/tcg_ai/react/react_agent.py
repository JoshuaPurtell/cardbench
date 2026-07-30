"""
ReAct agent for Pokemon TCG.

This module provides the LLM-based agent that selects actions based on
game observations.
"""

import json
import re
from dataclasses import dataclass, field
from typing import Any, Callable, Optional, List, Dict
import logging

from .prompts import SYSTEM_PROMPT, build_user_prompt
from .headless_game import GameObservation

logger = logging.getLogger(__name__)


@dataclass
class ReactAgentConfig:
    """Configuration for the ReAct agent."""

    # LLM settings
    model: str = "gpt-4"
    temperature: float = 0.3
    max_tokens: int = 512
    timeout: float = 30.0

    # System prompt
    system_prompt: str = SYSTEM_PROMPT

    # Behavior settings
    strict_to_random: bool = True
    include_reasoning: bool = True
    max_retries: int = 2

    # API settings (for OpenAI-compatible APIs)
    api_base: Optional[str] = None


@dataclass
class ParsedAction:
    """Parsed action from LLM response."""

    action_type: str
    card_id: Optional[int] = None
    target_id: Optional[int] = None
    energy_id: Optional[int] = None
    attack: Optional[str] = None
    card_ids: List[int] = field(default_factory=list)
    energy_ids: List[int] = field(default_factory=list)
    target_ids: List[int] = field(default_factory=list)
    power_name: Optional[str] = None
    reason: Optional[str] = None
    todo_add: List[str] = field(default_factory=list)
    raw_response: str = ""

    def to_action_dict(self) -> Dict[str, Any]:
        """Convert to action dictionary for game engine."""
        result = {"action": self.action_type}

        if self.card_id is not None:
            result["card_id"] = self.card_id
        if self.target_id is not None:
            result["target_id"] = self.target_id
        if self.energy_id is not None:
            result["energy_id"] = self.energy_id
        if self.attack:
            result["attack"] = self.attack
        if self.card_ids:
            result["card_ids"] = self.card_ids
        if self.energy_ids:
            result["energy_ids"] = self.energy_ids
        if self.target_ids:
            result["target_ids"] = self.target_ids
        if self.power_name:
            result["power_name"] = self.power_name

        return result


class ReactAgent:
    """ReAct-style agent for Pokemon TCG.

    This agent uses an LLM to select actions based on game observations.
    It maintains a TODO list for strategic planning and tracks action history.
    """

    def __init__(
        self,
        api_key: str,
        config: Optional[ReactAgentConfig] = None,
        llm_call_fn: Optional[Callable] = None,
    ):
        """Initialize the agent.

        Args:
            api_key: API key for the LLM provider.
            config: Agent configuration.
            llm_call_fn: Optional custom LLM call function.
                Should have signature: (system: str, user: str, config: ReactAgentConfig) -> str
        """
        self.api_key = api_key
        self.config = config or ReactAgentConfig()
        self._llm_call_fn = llm_call_fn

        # State
        self.todo_list: List[str] = [
            "Assess the board state",
            "Build up energy on Pokemon",
            "Take prize cards by knocking out opponent Pokemon",
        ]
        self.action_history: List[dict] = []
        self.last_error: Optional[str] = None

    async def select_action(
        self,
        observation: GameObservation,
        last_result: Optional[str] = None,
    ) -> ParsedAction:
        """Select an action based on the current observation.

        Args:
            observation: Current game observation.
            last_result: Optional result from the last action.

        Returns:
            Parsed action to execute.
        """
        # Build prompt
        game_state = observation.render()
        user_prompt = build_user_prompt(
            game_state=game_state,
            todo_list=self.todo_list,
            last_action_result=last_result,
        )

        # Try to get LLM response
        for attempt in range(self.config.max_retries + 1):
            try:
                response = await self._call_llm(user_prompt)
                parsed = self._parse_response(response, observation)

                # Update TODO list
                if parsed.todo_add:
                    self._update_todos(parsed.todo_add)

                # Record history
                self.action_history.append({
                    "observation_compact": observation.render_compact(),
                    "response": response,
                    "action": parsed.to_action_dict(),
                    "reason": parsed.reason,
                })

                self.last_error = None
                return parsed

            except Exception as e:
                logger.warning(f"LLM call failed (attempt {attempt + 1}): {e}")
                self.last_error = str(e)

                if attempt >= self.config.max_retries:
                    if self.config.strict_to_random:
                        return self._random_strict(observation)
                    raise

        # Should never reach here
        return self._random_strict(observation)

    async def _call_llm(self, user_prompt: str) -> str:
        """Call the LLM to get a response.

        Args:
            user_prompt: The user prompt to send.

        Returns:
            LLM response text.
        """
        if self._llm_call_fn:
            return await self._llm_call_fn(
                self.config.system_prompt,
                user_prompt,
                self.config,
            )

        # Default: Use OpenAI-compatible API
        return await self._call_openai_compatible(user_prompt)

    def _is_reasoning_model(self) -> bool:
        """Check if the model is a reasoning model (o1, o3, gpt-5, etc.)."""
        model = self.config.model.lower()
        reasoning_prefixes = ("o1", "o3", "gpt-5")
        return any(model.startswith(prefix) for prefix in reasoning_prefixes)

    async def _call_openai_compatible(self, user_prompt: str) -> str:
        """Call an OpenAI-compatible API.

        Args:
            user_prompt: The user prompt.

        Returns:
            Response text.
        """
        try:
            import httpx
        except ImportError:
            raise ImportError("httpx is required for API calls. Install with: pip install httpx")

        base_url = self.config.api_base or "https://api.openai.com/v1"

        # Build request based on model type
        is_reasoning = self._is_reasoning_model()

        if is_reasoning:
            # Reasoning models: no temperature, use max_completion_tokens, include system in user msg
            combined_prompt = f"{self.config.system_prompt}\n\n{user_prompt}"
            request_json = {
                "model": self.config.model,
                "max_completion_tokens": self.config.max_tokens,
                "messages": [
                    {"role": "user", "content": combined_prompt},
                ],
            }
        else:
            # Standard models
            request_json = {
                "model": self.config.model,
                "temperature": self.config.temperature,
                "max_tokens": self.config.max_tokens,
                "messages": [
                    {"role": "system", "content": self.config.system_prompt},
                    {"role": "user", "content": user_prompt},
                ],
            }

        async with httpx.AsyncClient(timeout=self.config.timeout) as client:
            response = await client.post(
                f"{base_url}/chat/completions",
                headers={
                    "Authorization": f"Bearer {self.api_key}",
                    "Content-Type": "application/json",
                },
                json=request_json,
            )
            response.raise_for_status()
            data = response.json()
            return data["choices"][0]["message"]["content"]

    def _parse_response(self, response: str, observation: GameObservation) -> ParsedAction:
        """Parse LLM response into an action.

        Args:
            response: Raw LLM response.
            observation: Current game observation (for context).

        Returns:
            Parsed action.
        """
        # Try to extract JSON from markdown code blocks first
        code_block_match = re.search(r'```(?:json)?\s*(\{.*?\})\s*```', response, re.DOTALL)
        if code_block_match:
            try:
                data = json.loads(code_block_match.group(1))
                return self._parse_json_action(data, response)
            except json.JSONDecodeError:
                pass

        # Try to find JSON object with proper brace matching
        json_str = self._extract_json_object(response)
        if json_str:
            try:
                data = json.loads(json_str)
                return self._parse_json_action(data, response)
            except json.JSONDecodeError:
                pass

        # Try to parse as natural language
        return self._parse_natural_language(response, observation)

    def _extract_json_object(self, text: str) -> Optional[str]:
        """Extract a JSON object from text with proper brace matching.

        Args:
            text: Text that may contain a JSON object.

        Returns:
            JSON string if found, None otherwise.
        """
        start = text.find('{')
        if start == -1:
            return None

        depth = 0
        in_string = False
        escape_next = False

        for i, char in enumerate(text[start:], start):
            if escape_next:
                escape_next = False
                continue

            if char == '\\' and in_string:
                escape_next = True
                continue

            if char == '"' and not escape_next:
                in_string = not in_string
                continue

            if in_string:
                continue

            if char == '{':
                depth += 1
            elif char == '}':
                depth -= 1
                if depth == 0:
                    return text[start:i + 1]

        return None

    def _parse_json_action(self, data: dict, raw_response: str) -> ParsedAction:
        """Parse a JSON action object.

        Args:
            data: Parsed JSON data.
            raw_response: Original response text.

        Returns:
            Parsed action.
        """
        action_type = data.get("action", "EndTurn")

        # Normalize action type
        action_type = self._normalize_action_type(action_type)

        return ParsedAction(
            action_type=action_type,
            card_id=data.get("card_id"),
            target_id=data.get("target_id"),
            energy_id=data.get("energy_id"),
            attack=data.get("attack") or data.get("attack_name"),
            card_ids=data.get("card_ids", []),
            energy_ids=data.get("energy_ids", []),
            target_ids=data.get("target_ids", []),
            power_name=data.get("power_name"),
            reason=data.get("reason"),
            todo_add=data.get("todo_add", []),
            raw_response=raw_response,
        )

    def _normalize_action_type(self, action_type: str) -> str:
        """Normalize action type to expected format.

        Args:
            action_type: Raw action type string.

        Returns:
            Normalized action type.
        """
        # Map common variations to canonical names
        mappings = {
            "play_basic": "PlayBasic",
            "playbasic": "PlayBasic",
            "attach_energy": "AttachEnergy",
            "attachenergy": "AttachEnergy",
            "evolve_from_hand": "EvolveFromHand",
            "evolvefromhand": "EvolveFromHand",
            "evolve": "EvolveFromHand",
            "play_trainer": "PlayTrainer",
            "playtrainer": "PlayTrainer",
            "use_power": "UsePower",
            "usepower": "UsePower",
            "declare_attack": "DeclareAttack",
            "declareattack": "DeclareAttack",
            "attack": "DeclareAttack",
            "end_turn": "EndTurn",
            "endturn": "EndTurn",
            "pass": "EndTurn",
            "choose_active": "ChooseActive",
            "chooseactive": "ChooseActive",
            "choose_bench": "ChooseBench",
            "choosebench": "ChooseBench",
            "choose_new_active": "ChooseNewActive",
            "choosenewactive": "ChooseNewActive",
            "take_cards_from_deck": "TakeCardsFromDeck",
            "takecardsfromdeck": "TakeCardsFromDeck",
            "take_cards_from_discard": "TakeCardsFromDiscard",
            "takecardsfromdiscard": "TakeCardsFromDiscard",
            "choose_pokemon_targets": "ChoosePokemonTargets",
            "choosepokemontargets": "ChoosePokemonTargets",
            "choose_attached_energy": "ChooseAttachedEnergy",
            "chooseattachedenergy": "ChooseAttachedEnergy",
            "discard_cards_from_hand": "DiscardCardsFromHand",
            "discardcardsfromhand": "DiscardCardsFromHand",
            "discard": "DiscardCardsFromHand",
            "retreat": "Retreat",
            "switch": "Retreat",
            "concede": "Concede",
            "cancel": "CancelPrompt",
            "cancel_prompt": "CancelPrompt",
        }

        normalized = mappings.get(action_type.lower(), action_type)
        return normalized

    def _parse_natural_language(
        self,
        response: str,
        observation: GameObservation,
    ) -> ParsedAction:
        """Parse natural language response.

        Args:
            response: Natural language response.
            observation: Current observation.

        Returns:
            Parsed action.
        """
        lower = response.lower()
        hints = observation.action_hints

        # End turn
        if "end turn" in lower or "pass" in lower:
            return ParsedAction(action_type="EndTurn", raw_response=response)

        # Attack
        if "attack" in lower:
            for attack in hints.usable_attacks:
                name = attack.get("name", "")
                if name.lower() in lower:
                    return ParsedAction(
                        action_type="DeclareAttack",
                        attack=name,
                        raw_response=response,
                    )
            if hints.usable_attacks:
                return ParsedAction(
                    action_type="DeclareAttack",
                    attack=hints.usable_attacks[0].get("name"),
                    raw_response=response,
                )

        # Play basic
        if "play basic" in lower or "play pokemon" in lower:
            if hints.playable_basic_ids:
                # Try to find ID in response
                for id in hints.playable_basic_ids:
                    if str(id) in response:
                        return ParsedAction(
                            action_type="PlayBasic",
                            card_id=id,
                            raw_response=response,
                        )
                return ParsedAction(
                    action_type="PlayBasic",
                    card_id=hints.playable_basic_ids[0],
                    raw_response=response,
                )

        # Attach energy
        if "attach energy" in lower:
            if hints.playable_energy_ids and hints.attach_targets:
                return ParsedAction(
                    action_type="AttachEnergy",
                    energy_id=hints.playable_energy_ids[0],
                    target_id=hints.attach_targets[0],
                    raw_response=response,
                )

        # Default: end turn if possible
        if hints.can_end_turn:
            return ParsedAction(action_type="EndTurn", raw_response=response)

        # Absolute strict
        return ParsedAction(action_type="EndTurn", raw_response=response)

    def _random_strict(self, observation: GameObservation) -> ParsedAction:
        """Generate a random strict action.

        Args:
            observation: Current observation.

        Returns:
            Random valid action.
        """
        import random

        hints = observation.action_hints
        actions = []

        # Collect available actions
        for id in hints.playable_basic_ids:
            actions.append(ParsedAction(action_type="PlayBasic", card_id=id))

        for energy_id in hints.playable_energy_ids:
            for target_id in hints.attach_targets:
                actions.append(ParsedAction(
                    action_type="AttachEnergy",
                    energy_id=energy_id,
                    target_id=target_id,
                ))

        for evo_id in hints.playable_evolution_ids:
            targets = hints.evolve_targets_by_card_id.get(evo_id, [])
            for target_id in targets:
                actions.append(ParsedAction(
                    action_type="EvolveFromHand",
                    card_id=evo_id,
                    target_id=target_id,
                ))

        for id in hints.playable_trainer_ids:
            actions.append(ParsedAction(action_type="PlayTrainer", card_id=id))

        if hints.can_declare_attack:
            for attack in hints.usable_attacks:
                actions.append(ParsedAction(
                    action_type="DeclareAttack",
                    attack=attack.get("name"),
                ))

        if hints.can_end_turn:
            actions.append(ParsedAction(action_type="EndTurn"))

        if actions:
            choice = random.choice(actions)
            choice.reason = "Random strict action"
            return choice

        return ParsedAction(action_type="EndTurn", reason="No valid actions available")

    def _update_todos(self, new_todos: List[str]) -> None:
        """Update the TODO list.

        Args:
            new_todos: New TODO items to add.
        """
        for todo in new_todos:
            if todo and todo not in self.todo_list:
                self.todo_list.append(todo)

        # Keep list manageable
        while len(self.todo_list) > 10:
            self.todo_list.pop(0)

    def get_history(self) -> List[dict]:
        """Get the action history."""
        return self.action_history.copy()

    def clear_history(self) -> None:
        """Clear the action history."""
        self.action_history.clear()

    def reset_todos(self) -> None:
        """Reset the TODO list to defaults."""
        self.todo_list = [
            "Assess the board state",
            "Build up energy on Pokemon",
            "Take prize cards by knocking out opponent Pokemon",
        ]
