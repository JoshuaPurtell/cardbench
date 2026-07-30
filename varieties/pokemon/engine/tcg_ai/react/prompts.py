"""
Prompt templates for the ReAct agent.
"""
from typing import List, Optional

SYSTEM_PROMPT = """You are an expert Pokemon TCG player. Your goal is to win the game by:
1. Taking all 6 of your opponent's prize cards, OR
2. Knocking out all of your opponent's Pokemon, OR
3. Making your opponent run out of cards to draw

CRITICAL: You must respond with ONLY a valid JSON object. No other text.

Response format:
{
    "action": "<action_type>",
    "card_id": <id>,
    "target_id": <id>,
    "energy_id": <id>,
    "attack": "<attack_name>",
    "card_ids": [<id1>, <id2>],
    "reason": "<brief reasoning>",
    "todo_add": ["<goal1>", "<goal2>"]
}

Available action types:

SETUP PHASE:
- ChooseActive: Choose your starting Active Pokemon (requires card_id)
- ChooseBench: Choose Basic Pokemon for your bench (requires card_ids)

MAIN PHASE (your turn):
- PlayBasic: Play a Basic Pokemon to your bench (requires card_id)
- AttachEnergy: Attach an energy card to a Pokemon (requires energy_id, target_id)
- EvolveFromHand: Evolve a Pokemon (requires card_id for evolution, target_id for target)
- PlayTrainer: Play a Trainer card (requires card_id)
- UsePower: Use a Poke-Power (requires source_id, power_name)
- Retreat: Switch your Active Pokemon with a benched one (requires to_bench_id)
- DeclareAttack: Declare an attack (requires attack name)
- EndTurn: End your turn

PROMPT RESPONSES:
- TakeCardsFromDeck: Choose cards from deck (requires card_ids)
- TakeCardsFromDiscard: Choose cards from discard (requires card_ids)
- ChoosePokemonTargets: Choose Pokemon targets (requires target_ids)
- ChooseAttachedEnergy: Choose energy to discard/move (requires energy_ids)
- DiscardCardsFromHand: Discard cards from hand (requires card_ids)
- ChooseNewActive: Choose new Active after KO (requires card_id)
- ChoosePrizeCards: Choose prize cards to take (requires card_ids)
- CancelPrompt: Cancel the current action (for optional effects)

STRATEGY TIPS:
1. Type matchups matter: Water beats Fire, Fire beats Grass, etc.
2. Weakness doubles damage, Resistance reduces it by 20-30
3. EX Pokemon give up 2 prizes when knocked out
4. Build up energy on bench Pokemon before they become active
5. Use Trainer cards for card advantage
6. Consider retreat cost when deciding on Active Pokemon

The card IDs in the game state are instance IDs (unique per card in play).
Always reference cards by their instance ID when taking actions.

Remember: Output ONLY the JSON object, nothing else."""


def build_user_prompt(
    game_state: str,
    todo_list: Optional[List[str]] = None,
    last_action_result: Optional[str] = None,
) -> str:
    """Build the user prompt from game state and context.

    Args:
        game_state: The rendered game state text.
        todo_list: Optional list of TODO items.
        last_action_result: Optional result from last action.

    Returns:
        The complete user prompt.
    """
    parts = []

    # Add game state
    parts.append(game_state)

    # Add TODO list if present
    if todo_list:
        parts.append("")
        parts.append("=== YOUR TODO LIST ===")
        for i, todo in enumerate(todo_list, 1):
            parts.append(f"{i}. {todo}")

    # Add last action result if present
    if last_action_result:
        parts.append("")
        parts.append(f"Last action result: {last_action_result}")

    # Add instruction
    parts.append("")
    parts.append("Choose your action. Respond with ONLY a JSON object.")

    return "\n".join(parts)


def build_prompt_response_template(prompt_type: str, options: list) -> str:
    """Build a prompt-specific instruction.

    Args:
        prompt_type: Type of prompt (e.g., "ChooseStartingActive").
        options: List of available options.

    Returns:
        Prompt-specific instruction.
    """
    templates = {
        "ChooseStartingActive": f"Choose your starting Active Pokemon from: {options}",
        "ChooseBenchBasics": f"Choose Basic Pokemon for your bench from: {options}",
        "ChooseAttack": f"Choose an attack: {options}",
        "ChooseNewActive": f"Choose a new Active Pokemon from your bench: {options}",
        "ChoosePrizeCards": f"Choose prize cards to take: {options}",
        "ChooseCardsFromDeck": f"Choose cards from your deck: {options}",
        "ChooseCardsFromDiscard": f"Choose cards from your discard pile: {options}",
        "ChoosePokemonInPlay": f"Choose Pokemon in play: {options}",
        "ChooseAttachedEnergy": f"Choose attached energy: {options}",
    }
    return templates.get(prompt_type, f"Respond to prompt with options: {options}")
