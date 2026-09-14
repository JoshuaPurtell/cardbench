//! Compose Crystal Guardians engine add-on + card fragments.

use tcg_core::runtime_hooks::{AttackOverrides, RuntimeHooks};
use tcg_core::{Attack, CardDefId, CardInstanceId, GameState};

use super::cards as remaining_cards;
use super::engine;
use crate::cg::runtime as cards;

pub fn create() -> RuntimeHooks {
    RuntimeHooks {
        attack_overrides,
        attack_cost_modifier: cards::attack_cost_modifier,
        post_attack: cards::post_attack,
        between_turns: cards::between_turns,
        execute_power: cards::execute_power,
        register_triggers: cards::register_triggers,
        apply_tool_stadium_effects: cards::apply_tool_stadium_effects,
        can_attach_tool: cards::can_attach_tool,
        can_attach_energy: |_, _, _, _| true,
        energy_units_override: |_, _, _, units| units,
        on_tool_attached: cards::on_tool_attached,
        energy_provides_override: cards::energy_provides_override,
        on_energy_attached: cards::on_energy_attached,
        after_attack: cards::after_attack,
        can_use_pokepower_override: cards::can_use_pokepower_override,
        is_pokebody_active_override: cards::is_pokebody_active_override,
        resolve_custom_prompt: cards::resolve_custom_prompt,
        power_effect_id_for: cards::power_effect_id,
        power_is_once_per_turn: cards::power_is_once_per_turn,
        card_has_power_or_body,
        is_double_rainbow: engine::is_double_rainbow,
        prevents_attack_effects: engine::prevents_attack_effects,
        tool_discard_timing_override: engine::tool_discard_timing_override,
        energy_units: engine::energy_units,
        retreat_cost_override,
        treats_pokemon_as_delta: |_, _| false,
        prevents_special_conditions: |_, _| false,
    }
}

fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = engine::attack_overrides(game, attack, attacker_id, defender_id);
    overrides.merge(remaining_cards::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cards::attack_overrides(game, attack, attacker_id, defender_id));
    overrides
}

fn retreat_cost_override(_game: &GameState, _pokemon_id: CardInstanceId, base: i32) -> i32 {
    base
}

fn card_has_power_or_body(def_id: &CardDefId) -> bool {
    crate::cg::CG_POWERS
        .iter()
        .any(|spec| match_number(def_id, "CG", spec.number))
}

fn match_number(def_id: &CardDefId, set_code: &str, number: &str) -> bool {
    number
        .parse::<u32>()
        .ok()
        .map(|value| tcg_core::runtime_hooks::def_id_matches(def_id, set_code, value))
        .unwrap_or(false)
}
