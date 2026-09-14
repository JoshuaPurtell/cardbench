//! Compose Holon Phantoms hooks. Gold is still a placeholder.

use tcg_core::runtime_hooks::RuntimeHooks;
use tcg_core::CardDefId;

use super::runtime;

pub fn create() -> RuntimeHooks {
    RuntimeHooks {
        attack_overrides: runtime::attack_overrides,
        attack_cost_modifier: runtime::attack_cost_modifier,
        post_attack: runtime::post_attack,
        between_turns: runtime::between_turns,
        execute_power: runtime::execute_power,
        register_triggers: runtime::register_triggers,
        apply_tool_stadium_effects: runtime::apply_tool_stadium_effects,
        can_attach_tool: runtime::can_attach_tool,
        can_attach_energy: |_, _, _, _| true,
        energy_units_override: |_, _, _, units| units,
        on_tool_attached: runtime::on_tool_attached,
        energy_provides_override: runtime::energy_provides_override,
        on_energy_attached: runtime::on_energy_attached,
        after_attack: runtime::after_attack,
        can_use_pokepower_override: runtime::can_use_pokepower_override,
        is_pokebody_active_override: runtime::is_pokebody_active_override,
        resolve_custom_prompt: runtime::resolve_custom_prompt,
        power_effect_id_for: runtime::power_effect_id,
        power_is_once_per_turn: runtime::power_is_once_per_turn,
        card_has_power_or_body,
        is_double_rainbow: |_| false,
        prevents_attack_effects: |_, _, _| false,
        tool_discard_timing_override: |_, _| None,
        energy_units: |_, _, _, _| 1,
        retreat_cost_override: |_, _, base| base,
        treats_pokemon_as_delta: |_, _| false,
        prevents_special_conditions: |_, _| false,
    }
}

fn card_has_power_or_body(def_id: &CardDefId) -> bool {
    crate::hp::HP_POWERS
        .iter()
        .any(|spec| match_number(def_id, "HP", spec.number))
}

fn match_number(def_id: &CardDefId, set_code: &str, number: &str) -> bool {
    number
        .parse::<u32>()
        .ok()
        .map(|value| tcg_core::runtime_hooks::def_id_matches(def_id, set_code, value))
        .unwrap_or(false)
}
