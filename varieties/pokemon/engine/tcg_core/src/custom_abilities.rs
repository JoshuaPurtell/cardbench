//! Custom abilities module that delegates to RuntimeHooks.
//!
//! This module provides the public API for expansion-specific behavior.
//! All implementations delegate to the hooks registered on the GameState.

use crate::ids::CardInstanceId;
use crate::{Attack, CardDefId, GameState, PokemonSlot, Type};
use crate::types::EnergyAttachmentSource;

// Re-export AttackOverrides from runtime_hooks for backwards compatibility
pub use crate::runtime_hooks::AttackOverrides;

pub fn apply_attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = (game.hooks().attack_overrides)(game, attack, attacker_id, defender_id);
    if game.ignore_weakness_for(attacker_id, defender_id) {
        overrides.ignore_weakness = true;
    }
    if game.ignore_resistance_for(attacker_id, defender_id) {
        overrides.ignore_resistance = true;
    }
    if game.prevent_damage_for(attacker_id, defender_id) {
        overrides.prevent_damage = true;
    }
    overrides
}

pub fn attack_cost_modifier(game: &GameState, attacker_id: CardInstanceId, attack: &Attack) -> i32 {
    (game.hooks().attack_cost_modifier)(game, attacker_id, attack)
}

pub fn apply_post_attack_custom(
    game: &mut GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
    damage_dealt: u16,
) {
    (game.hooks().post_attack)(game, attacker_id, defender_id, damage_dealt);
}

pub fn apply_between_turns_custom(game: &mut GameState) {
    (game.hooks().between_turns)(game);
}

pub fn execute_custom_power(
    game: &mut GameState,
    power_name: &str,
    source_id: CardInstanceId,
) -> bool {
    (game.hooks().execute_power)(game, power_name, source_id)
}

pub fn register_card_triggers(game: &mut GameState, slot: &PokemonSlot) {
    (game.hooks().register_triggers)(game, slot);
}

fn apply_attached_tool_type_effects(game: &mut GameState) {
    let mut updates: Vec<(CardInstanceId, Vec<Type>)> = Vec::new();
    for player in &game.players {
        for slot in player.active.iter().chain(player.bench.iter()) {
            let printed = game
                .card_meta(&slot.card.def_id)
                .map(|meta| meta.types.clone())
                .unwrap_or_else(|| slot.types.clone());
            let mut types = printed;
            if let Some(tool) = slot.attached_tool.as_ref() {
                if let Some(effect) = game
                    .card_meta(&tool.def_id)
                    .and_then(|meta| meta.trainer_effect.clone())
                {
                    if let Some(arr) = effect.get("types").and_then(|value| value.as_array()) {
                        let parsed: Vec<Type> = arr
                            .iter()
                            .filter_map(|value| serde_json::from_value(value.clone()).ok())
                            .collect();
                        if !parsed.is_empty() {
                            types = parsed;
                        }
                    }
                }
            }
            updates.push((slot.card.id, types));
        }
    }
    for (id, types) in updates {
        if let Some(slot) = game.find_pokemon_slot_mut(id) {
            slot.types = types;
        }
    }
}

pub fn apply_tool_stadium_effects(game: &mut GameState) {
    apply_attached_tool_type_effects(game);
    (game.hooks().apply_tool_stadium_effects)(game);
}

pub fn can_attach_tool(
    game: &GameState,
    player: crate::PlayerId,
    tool_id: CardInstanceId,
    target_id: CardInstanceId,
) -> bool {
    (game.hooks().can_attach_tool)(game, player, tool_id, target_id)
}

pub fn can_attach_energy(
    game: &GameState,
    player: crate::PlayerId,
    energy_id: CardInstanceId,
    target_id: CardInstanceId,
) -> bool {
    (game.hooks().can_attach_energy)(game, player, energy_id, target_id)
}

pub fn on_tool_attached(
    game: &mut GameState,
    tool_id: CardInstanceId,
    target_id: CardInstanceId,
) {
    (game.hooks().on_tool_attached)(game, tool_id, target_id);
}

pub fn energy_provides_override(
    game: &GameState,
    card: &crate::CardInstance,
) -> Option<Vec<Type>> {
    (game.hooks().energy_provides_override)(game, card)
}

pub fn on_energy_attached(
    game: &mut GameState,
    energy_id: CardInstanceId,
    target_id: CardInstanceId,
    source: EnergyAttachmentSource,
) {
    (game.hooks().on_energy_attached)(game, energy_id, target_id, source);
}

pub fn after_attack(game: &mut GameState, attacker_id: CardInstanceId, defender_id: CardInstanceId) {
    (game.hooks().after_attack)(game, attacker_id, defender_id);
}

pub fn can_use_pokepower_override(
    game: &GameState,
    player: crate::PlayerId,
    pokemon_id: CardInstanceId,
) -> Option<bool> {
    (game.hooks().can_use_pokepower_override)(game, player, pokemon_id)
}

pub fn is_pokebody_active_override(
    game: &GameState,
    player: crate::PlayerId,
    pokemon_id: CardInstanceId,
) -> Option<bool> {
    (game.hooks().is_pokebody_active_override)(game, player, pokemon_id)
}

pub fn resolve_custom_prompt(
    game: &mut GameState,
    effect_id: &str,
    source_id: Option<CardInstanceId>,
    target_ids: &[CardInstanceId],
) -> bool {
    (game.hooks().resolve_custom_prompt)(game, effect_id, source_id, target_ids)
}

pub fn power_effect_id_for(def_id: &CardDefId, power_name: &str) -> Option<String> {
    // This is a static lookup that doesn't require game state.
    // We use a default hooks instance for this.
    // Note: In a fully modular system, this would require passing hooks explicitly.
    // For now, we use tcg_expansions::create_hooks() equivalent logic.
    crate::runtime_hooks::RuntimeHooks::default().power_effect_id_for.clone()(def_id, power_name)
}

pub fn power_is_once_per_turn(def_id: &CardDefId, power_name: &str) -> bool {
    // This is a static lookup that doesn't require game state.
    (crate::runtime_hooks::RuntimeHooks::default().power_is_once_per_turn)(def_id, power_name)
}

// Re-export def_id_matches from runtime_hooks for backwards compatibility
pub use crate::runtime_hooks::def_id_matches;

pub fn card_has_power_or_body(def_id: &CardDefId) -> bool {
    // This is a static lookup that doesn't require game state.
    (crate::runtime_hooks::RuntimeHooks::default().card_has_power_or_body)(def_id)
}
