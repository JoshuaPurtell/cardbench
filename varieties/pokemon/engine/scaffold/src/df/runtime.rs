//! Dragon Frontiers runtime merge facade.
//!
//! Set-wide rules live in `engine.rs`. Per-card bodies live in `cards/`.
//! Trainers live in `trainers.rs`. This module only merges those layers.

use tcg_core::runtime_hooks::AttackOverrides;
use tcg_core::{
    Attack, CardDefId, CardInstance, CardInstanceId, EnergyAttachmentSource, GameState, PlayerId,
    PokemonSlot, Type,
};

pub fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = super::engine::attack_overrides(game, attack, attacker_id, defender_id);
    overrides.merge(super::cards::attack_overrides(
        game,
        attack,
        attacker_id,
        defender_id,
    ));
    overrides
}

pub fn post_attack(
    _game: &mut GameState,
    _attacker_id: CardInstanceId,
    _defender_id: CardInstanceId,
    _damage_dealt: u16,
) {
}

pub fn between_turns(game: &mut GameState) {
    for owner in [PlayerId::P1, PlayerId::P2] {
        if super::cards::snorlax::is_snorlax_asleep_for_bedhead(game, owner) {
            let opponent = match owner {
                PlayerId::P1 => PlayerId::P2,
                PlayerId::P2 => PlayerId::P1,
            };
            game.deal_between_turns_damage(opponent, super::cards::snorlax::BEDHEAD_DAMAGE);
        }
    }
    super::engine::discard_boost_energy(game);
}

pub fn execute_power(game: &mut GameState, power_name: &str, source_id: CardInstanceId) -> bool {
    if super::cards::execute_power(game, power_name, source_id) {
        return true;
    }
    super::trainers::execute_power(game, power_name, source_id)
}

pub fn power_effect_id(def_id: &CardDefId, power_name: &str) -> Option<String> {
    super::cards::power_effect_id(def_id, power_name)
}

pub fn power_is_once_per_turn(def_id: &CardDefId, power_name: &str) -> bool {
    super::cards::power_is_once_per_turn(def_id, power_name)
}

pub fn apply_tool_stadium_effects(_game: &mut GameState) {}

pub fn can_attach_tool(
    _game: &GameState,
    _player: PlayerId,
    _tool_id: CardInstanceId,
    _target_id: CardInstanceId,
) -> bool {
    true
}

pub fn on_tool_attached(
    _game: &mut GameState,
    _tool_id: CardInstanceId,
    _target_id: CardInstanceId,
) {
}

pub fn energy_provides_override(game: &GameState, card: &CardInstance) -> Option<Vec<Type>> {
    super::engine::energy_provides_override(game, card)
}

pub fn on_energy_attached(
    game: &mut GameState,
    energy_id: CardInstanceId,
    target_id: CardInstanceId,
    source: EnergyAttachmentSource,
) {
    super::engine::on_energy_attached(game, energy_id, target_id, source);
}

pub fn after_attack(
    game: &mut GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) {
    super::cards::after_attack(game, attacker_id, defender_id);
}

pub fn can_use_pokepower_override(
    game: &GameState,
    player: PlayerId,
    pokemon_id: CardInstanceId,
) -> Option<bool> {
    super::engine::can_use_pokepower_override(game, player, pokemon_id)
}

pub fn is_pokebody_active_override(
    _game: &GameState,
    _player: PlayerId,
    _pokemon_id: CardInstanceId,
) -> Option<bool> {
    None
}

pub fn register_triggers(game: &mut GameState, slot: &PokemonSlot) {
    super::cards::register_triggers(game, slot);
}

pub fn resolve_custom_prompt(
    game: &mut GameState,
    effect_id: &str,
    source_id: Option<CardInstanceId>,
    target_ids: &[CardInstanceId],
) -> bool {
    if super::cards::resolve_custom_prompt(game, effect_id, source_id, target_ids) {
        return true;
    }
    super::trainers::resolve_custom_prompt(game, effect_id, source_id, target_ids)
}

pub fn attack_cost_modifier(
    game: &GameState,
    attacker_id: CardInstanceId,
    attack: &Attack,
) -> i32 {
    super::cards::attack_cost_modifier(game, attacker_id, attack)
}
