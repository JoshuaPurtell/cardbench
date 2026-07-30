"""
Headless game environment for Pokemon TCG.

This module provides a text-based interface to the Pokemon TCG game engine,
suitable for LLM-based agents.
"""

from dataclasses import dataclass, field
from typing import Any, Optional, List, Dict
from enum import Enum
import json


class Phase(str, Enum):
    """Game phases."""
    SETUP = "Setup"
    START_OF_TURN = "StartOfTurn"
    DRAW = "Draw"
    MAIN = "Main"
    ATTACK = "Attack"
    END_OF_TURN = "EndOfTurn"
    BETWEEN_TURNS = "BetweenTurns"


class SpecialCondition(str, Enum):
    """Special conditions."""
    POISONED = "Poisoned"
    BURNED = "Burned"
    ASLEEP = "Asleep"
    PARALYZED = "Paralyzed"
    CONFUSED = "Confused"


@dataclass
class PokemonView:
    """View of a Pokemon in play."""
    card_id: int
    def_id: str
    name: str
    hp: int
    damage_counters: int
    types: List[str]
    weakness: Optional[dict] = None
    resistance: Optional[dict] = None
    attached_energy: List[dict] = field(default_factory=list)
    attached_tool: Optional[dict] = None
    special_conditions: List[str] = field(default_factory=list)
    is_ex: bool = False
    is_star: bool = False

    @property
    def remaining_hp(self) -> int:
        return max(0, self.hp - self.damage_counters * 10)

    def render(self, label: str = "Pokemon") -> str:
        """Render this Pokemon to text."""
        lines = []
        type_str = "/".join(self.types) if self.types else "Unknown"
        ex_star = " [EX]" if self.is_ex else (" [Star]" if self.is_star else "")

        lines.append(f"{label}: {self.name} ({type_str}){ex_star}")
        lines.append(f"  ID: {self.card_id}")
        lines.append(f"  HP: {self.remaining_hp}/{self.hp}")

        if self.attached_energy:
            energy_list = [f"{e['def_id']} (id:{e['id']})" for e in self.attached_energy]
            lines.append(f"  Energy: {', '.join(energy_list)}")
        else:
            lines.append("  Energy: None")

        if self.attached_tool:
            lines.append(f"  Tool: {self.attached_tool['def_id']} (id:{self.attached_tool['id']})")

        if self.weakness:
            lines.append(f"  Weakness: {self.weakness['type']} x{self.weakness['multiplier']}")

        if self.resistance:
            lines.append(f"  Resistance: {self.resistance['type']} -{self.resistance['value']}")

        if self.special_conditions:
            lines.append(f"  Status: {', '.join(self.special_conditions)}")

        return "\n".join(lines)


@dataclass
class ActionHints:
    """Hints about available actions."""
    playable_basic_ids: List[int] = field(default_factory=list)
    playable_energy_ids: List[int] = field(default_factory=list)
    playable_trainer_ids: List[int] = field(default_factory=list)
    playable_evolution_ids: List[int] = field(default_factory=list)
    evolve_targets_by_card_id: Dict[int, List[int]] = field(default_factory=dict)
    attach_targets: List[int] = field(default_factory=list)
    can_declare_attack: bool = False
    can_end_turn: bool = False
    usable_attacks: List[dict] = field(default_factory=list)


@dataclass
class GameObservation:
    """Observation of the game state for a player."""
    player_id: str
    phase: Phase
    current_player: str
    my_hand: List[dict]
    my_deck_count: int
    my_discard: List[dict]
    my_prizes_count: int
    my_active: Optional[PokemonView]
    my_bench: List[PokemonView]
    opponent_hand_count: int
    opponent_deck_count: int
    opponent_prizes_count: int
    opponent_active: Optional[PokemonView]
    opponent_bench: List[PokemonView]
    stadium_in_play: Optional[dict]
    action_hints: ActionHints
    pending_prompt: Optional[dict] = None
    terminated: bool = False
    winner: Optional[str] = None

    def render(self) -> str:
        """Render the full game state to text."""
        lines = []

        # Header
        lines.append("=== POKEMON TCG GAME STATE ===")
        lines.append(f"Phase: {self.phase.value} | Current Turn: {self.current_player} | You are: {self.player_id}")
        lines.append("")

        # Pending prompt
        if self.pending_prompt:
            lines.append(">>> PENDING ACTION REQUIRED <<<")
            lines.append(self._render_prompt(self.pending_prompt))
            lines.append("")

        # Your side
        lines.append("=== YOUR SIDE ===")

        if self.my_active:
            lines.append(self.my_active.render("Active"))
        else:
            lines.append("Active: None")
        lines.append("")

        lines.append("Bench:")
        if self.my_bench:
            for i, pokemon in enumerate(self.my_bench):
                lines.append(pokemon.render(f"Bench {i+1}"))
        else:
            lines.append("  (empty)")
        lines.append("")

        lines.append("Hand:")
        if self.my_hand:
            for card in self.my_hand:
                lines.append(f"  - {card['def_id']} (id:{card['id']})")
        else:
            lines.append("  (empty)")
        lines.append("")

        lines.append(f"Deck: {self.my_deck_count} cards | Prizes: {self.my_prizes_count} remaining")
        if self.my_discard:
            lines.append(f"Discard: {len(self.my_discard)} cards")
        lines.append("")

        # Opponent side
        lines.append("=== OPPONENT SIDE ===")

        if self.opponent_active:
            lines.append(self.opponent_active.render("Active"))
        else:
            lines.append("Active: None")
        lines.append("")

        lines.append("Bench:")
        if self.opponent_bench:
            for i, pokemon in enumerate(self.opponent_bench):
                lines.append(pokemon.render(f"Bench {i+1}"))
        else:
            lines.append("  (empty)")
        lines.append("")

        lines.append(f"Hand: {self.opponent_hand_count} cards | Deck: {self.opponent_deck_count} cards | Prizes: {self.opponent_prizes_count} remaining")
        lines.append("")

        # Stadium
        if self.stadium_in_play:
            lines.append(f"Stadium in Play: {self.stadium_in_play['def_id']}")
            lines.append("")

        # Available actions
        lines.append("=== AVAILABLE ACTIONS ===")
        lines.append(self._render_available_actions())

        return "\n".join(lines)

    def render_compact(self) -> str:
        """Render a compact version of the game state."""
        parts = []

        if self.my_active:
            parts.append(f"My Active: {self.my_active.name} HP:{self.my_active.remaining_hp}/{self.my_active.hp}")

        parts.append(f"Bench: {len(self.my_bench)}")

        if self.opponent_active:
            parts.append(f"Opp Active: {self.opponent_active.name} HP:{self.opponent_active.remaining_hp}/{self.opponent_active.hp}")

        parts.append(f"Prizes: {self.my_prizes_count}/{self.opponent_prizes_count}")
        parts.append(f"Phase: {self.phase.value}")

        if self.pending_prompt:
            parts.append(f"Prompt: {self.pending_prompt.get('type', 'Unknown')}")

        return " | ".join(parts)

    def _render_prompt(self, prompt: dict) -> str:
        """Render a prompt to text."""
        prompt_type = prompt.get("type", "Unknown")
        options = prompt.get("options", [])

        if prompt_type == "ChooseStartingActive":
            ids = [str(o) for o in options]
            return f"Choose your starting Active Pokemon from: [{', '.join(ids)}]"
        elif prompt_type == "ChooseBenchBasics":
            ids = [str(o) for o in options]
            min_val = prompt.get("min", 0)
            max_val = prompt.get("max", len(options))
            return f"Choose {min_val}-{max_val} Basic Pokemon for your Bench from: [{', '.join(ids)}]"
        elif prompt_type == "ChooseAttack":
            attacks = prompt.get("attacks", [])
            attack_names = [a.get("name", "Unknown") for a in attacks]
            return f"Choose an attack: [{', '.join(attack_names)}]"
        elif prompt_type == "ChooseNewActive":
            ids = [str(o) for o in options]
            return f"Choose a new Active Pokemon from bench: [{', '.join(ids)}]"
        elif prompt_type == "ChoosePrizeCards":
            ids = [str(o) for o in options]
            min_val = prompt.get("min", 1)
            max_val = prompt.get("max", len(options))
            return f"Choose {min_val}-{max_val} prize cards: [{', '.join(ids)}]"
        else:
            return f"{prompt_type}: {prompt}"

    def _render_available_actions(self) -> str:
        """Render available actions."""
        hints = self.action_hints
        lines = []

        if hints.playable_basic_ids:
            cards = [f"id:{id}" for id in hints.playable_basic_ids]
            lines.append(f"  Play Basic Pokemon: {', '.join(cards)}")

        if hints.playable_energy_ids and hints.attach_targets:
            energy = [f"id:{id}" for id in hints.playable_energy_ids]
            lines.append(f"  Attach Energy: {', '.join(energy)} -> targets available")

        if hints.playable_evolution_ids:
            for evo_id in hints.playable_evolution_ids:
                targets = hints.evolve_targets_by_card_id.get(evo_id, [])
                target_ids = [str(t) for t in targets]
                lines.append(f"  Evolve: id:{evo_id} -> targets: [{', '.join(target_ids)}]")

        if hints.playable_trainer_ids:
            trainers = [f"id:{id}" for id in hints.playable_trainer_ids]
            lines.append(f"  Play Trainer: {', '.join(trainers)}")

        if hints.can_declare_attack and hints.usable_attacks:
            lines.append("  Attacks available:")
            for attack in hints.usable_attacks:
                name = attack.get("name", "Unknown")
                damage = attack.get("damage", 0)
                cost = attack.get("cost", {})
                total = cost.get("total_energy", 0)
                effect = " (has effect)" if attack.get("effect_ast") else ""
                lines.append(f"    - {name}: {damage} damage, Cost: {total} energy{effect}")

        if hints.can_end_turn:
            lines.append("  End Turn: Available")

        if not lines:
            lines.append("  (no actions available - waiting for prompt response)")

        return "\n".join(lines)


class HeadlessGame:
    """Headless Pokemon TCG game environment.

    This class wraps the Rust game engine and provides a text-based interface
    for LLM agents to play the game.

    Note: This is a placeholder implementation. In production, this would
    interface with the Rust game engine via PyO3 or subprocess communication.
    """

    def __init__(
        self,
        seed: int = 42,
        p1_deck: Optional[List[str]] = None,
        p2_deck: Optional[List[str]] = None,
        ai_opponent_version: int = 4,
    ):
        """Initialize a headless game.

        Args:
            seed: Random seed for reproducibility.
            p1_deck: Player 1's deck (list of card def IDs).
            p2_deck: Player 2's deck (list of card def IDs).
            ai_opponent_version: Version of the AI opponent (1-4).
        """
        self.seed = seed
        self.p1_deck = p1_deck or []
        self.p2_deck = p2_deck or []
        self.ai_opponent_version = ai_opponent_version
        self._game_state: dict = {}
        self._step_count = 0
        self._terminated = False
        self._winner: Optional[str] = None

    async def initialize(self) -> GameObservation:
        """Initialize the game and return the initial observation.

        Returns:
            Initial game observation for player 1.
        """
        # This would call into the Rust engine
        # For now, return a placeholder
        self._step_count = 0
        self._terminated = False
        self._winner = None

        # In production, this would:
        # 1. Create a GameState with the decks
        # 2. Run the initial setup phase
        # 3. Return the observation

        return self._build_observation()

    async def step(self, action: dict) -> tuple[GameObservation, float, bool, dict]:
        """Execute an action and return the new observation.

        Args:
            action: The action to execute (parsed from LLM response).

        Returns:
            Tuple of (observation, reward, terminated, info).
        """
        self._step_count += 1

        # This would:
        # 1. Parse the action dict into an Action enum
        # 2. Call game.apply_action()
        # 3. Step the game until next player action needed
        # 4. Return observation

        # Placeholder reward calculation
        reward = 0.0
        info = {"step": self._step_count}

        return self._build_observation(), reward, self._terminated, info

    async def run_ai_opponent_turn(self) -> None:
        """Run the AI opponent's turn.

        This advances the game state through the opponent's actions.
        """
        # This would:
        # 1. Get the opponent's view
        # 2. Call the AI controller's methods
        # 3. Apply actions until turn ends
        pass

    def _build_observation(self) -> GameObservation:
        """Build an observation from current game state."""
        # Placeholder - in production this comes from Rust
        return GameObservation(
            player_id="P1",
            phase=Phase.MAIN,
            current_player="P1",
            my_hand=[],
            my_deck_count=60,
            my_discard=[],
            my_prizes_count=6,
            my_active=None,
            my_bench=[],
            opponent_hand_count=7,
            opponent_deck_count=53,
            opponent_prizes_count=6,
            opponent_active=None,
            opponent_bench=[],
            stadium_in_play=None,
            action_hints=ActionHints(),
            pending_prompt=None,
            terminated=self._terminated,
            winner=self._winner,
        )

    @property
    def is_terminated(self) -> bool:
        """Check if the game is over."""
        return self._terminated

    @property
    def winner(self) -> Optional[str]:
        """Get the winner if game is over."""
        return self._winner

    @property
    def step_count(self) -> int:
        """Get the current step count."""
        return self._step_count


def parse_game_view_json(view_json: dict) -> GameObservation:
    """Parse a GameView JSON from Rust into a GameObservation.

    Args:
        view_json: JSON representation of GameView from Rust.

    Returns:
        GameObservation object.
    """
    def parse_pokemon(data: Optional[dict]) -> Optional[PokemonView]:
        if not data:
            return None
        card = data.get("card", {})
        return PokemonView(
            card_id=card.get("id", 0),
            def_id=card.get("def_id", ""),
            name=card.get("def_id", "").split("-")[-1] if card.get("def_id") else "Unknown",
            hp=data.get("hp", 0),
            damage_counters=data.get("damage_counters", 0),
            types=[t for t in data.get("types", [])],
            weakness=data.get("weakness"),
            resistance=data.get("resistance"),
            attached_energy=[
                {"id": e.get("id"), "def_id": e.get("def_id")}
                for e in data.get("attached_energy", [])
            ],
            attached_tool=data.get("attached_tool"),
            special_conditions=data.get("special_conditions", []),
            is_ex=data.get("is_ex", False),
            is_star=data.get("is_star", False),
        )

    def parse_action_hints(data: dict) -> ActionHints:
        return ActionHints(
            playable_basic_ids=data.get("playable_basic_ids", []),
            playable_energy_ids=data.get("playable_energy_ids", []),
            playable_trainer_ids=data.get("playable_trainer_ids", []),
            playable_evolution_ids=data.get("playable_evolution_ids", []),
            evolve_targets_by_card_id=data.get("evolve_targets_by_card_id", {}),
            attach_targets=data.get("attach_targets", []),
            can_declare_attack=data.get("can_declare_attack", False),
            can_end_turn=data.get("can_end_turn", False),
            usable_attacks=data.get("usable_attacks", []),
        )

    return GameObservation(
        player_id=view_json.get("player_id", "P1"),
        phase=Phase(view_json.get("phase", "Main")),
        current_player=view_json.get("current_player", "P1"),
        my_hand=[
            {"id": c.get("id"), "def_id": c.get("def_id")}
            for c in view_json.get("my_hand", [])
        ],
        my_deck_count=view_json.get("my_deck_count", 0),
        my_discard=[
            {"id": c.get("id"), "def_id": c.get("def_id")}
            for c in view_json.get("my_discard", [])
        ],
        my_prizes_count=view_json.get("my_prizes_count", 6),
        my_active=parse_pokemon(view_json.get("my_active")),
        my_bench=[parse_pokemon(p) for p in view_json.get("my_bench", []) if p],
        opponent_hand_count=view_json.get("opponent_hand_count", 0),
        opponent_deck_count=view_json.get("opponent_deck_count", 0),
        opponent_prizes_count=view_json.get("opponent_prizes_count", 6),
        opponent_active=parse_pokemon(view_json.get("opponent_active")),
        opponent_bench=[parse_pokemon(p) for p in view_json.get("opponent_bench", []) if p],
        stadium_in_play=view_json.get("stadium_in_play"),
        action_hints=parse_action_hints(view_json.get("action_hints", {})),
        pending_prompt=view_json.get("pending_prompt"),
    )
