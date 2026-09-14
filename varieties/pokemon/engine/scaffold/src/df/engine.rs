//! Dragon Frontiers set-wide engine add-on.
//!
//! Boost Energy, Scramble Energy, Holon Veil, Holon Energy FF/GL/WP, and
//! Holon Legacy. Per-Pokémon attack bodies live in `df/cards/`.

use tcg_core::runtime_hooks::{def_id_matches, AttackOverrides};
use tcg_core::{
    Attack, CardInstance, CardInstanceId, EnergyAttachmentSource, GameState, Marker, PlayerId,
    PokemonSlot, Stage, Type,
};

pub fn energy_units(
    game: &GameState,
    pokemon_id: CardInstanceId,
    energy: &CardInstance,
    _provides: &[Type],
) -> usize {
    if def_id_matches(&energy.def_id, "DF", 87) {
        return 3;
    }
    if def_id_matches(&energy.def_id, "DF", 89) {
        if let Some(slot) = game.slot_by_id(pokemon_id) {
            if scramble_energy_active(game, slot) {
                return 3;
            }
        }
    }
    1
}

pub fn treats_pokemon_as_delta(game: &GameState, player: PlayerId) -> bool {
    let player_state = match player {
        PlayerId::P1 => &game.players[0],
        PlayerId::P2 => &game.players[1],
    };
    player_state
        .active
        .iter()
        .chain(player_state.bench.iter())
        .any(|slot| def_id_matches(&slot.card.def_id, "DF", 1))
}

pub fn prevents_attack_effects(
    game: &GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> bool {
    holon_energy_wp_prevents_attack_effects(game, attacker_id, defender_id)
}

pub fn prevents_special_conditions(game: &GameState, pokemon_id: CardInstanceId) -> bool {
    holon_energy_gl_prevents_special_conditions(game, pokemon_id)
}

pub fn retreat_cost_override(game: &GameState, pokemon_id: CardInstanceId, base: i32) -> i32 {
    if base <= 0 {
        return base;
    }
    let Some(slot) = game.slot_by_id(pokemon_id) else {
        return base;
    };
    if holon_energy_wp_reduces_retreat(game, slot) {
        0
    } else {
        base
    }
}

fn scramble_energy_active(game: &GameState, slot: &PokemonSlot) -> bool {
    if slot.stage == Stage::Basic || slot.is_ex {
        return false;
    }
    let Some(owner) = game.owner_for_pokemon(slot.card.id) else {
        return false;
    };
    let (me, opp) = match owner {
        PlayerId::P1 => (&game.players[0], &game.players[1]),
        PlayerId::P2 => (&game.players[1], &game.players[0]),
    };
    me.prizes.count() > opp.prizes.count()
}

fn holon_energy_wp_prevents_attack_effects(
    game: &GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> bool {
    let Some(attacker_owner) = game.owner_for_pokemon(attacker_id) else {
        return false;
    };
    let Some(defender_owner) = game.owner_for_pokemon(defender_id) else {
        return false;
    };
    if attacker_owner == defender_owner {
        return false;
    }
    let Some(slot) = game.slot_by_id(defender_id) else {
        return false;
    };
    if slot.is_ex {
        return false;
    }
    if !has_named_energy(game, slot, "Holon Energy WP") {
        return false;
    }
    has_basic_energy_type(game, slot, Type::Water)
}

fn holon_energy_gl_prevents_special_conditions(
    game: &GameState,
    pokemon_id: CardInstanceId,
) -> bool {
    let Some(slot) = game.slot_by_id(pokemon_id) else {
        return false;
    };
    if slot.is_ex {
        return false;
    }
    if !has_named_energy(game, slot, "Holon Energy GL") {
        return false;
    }
    has_basic_energy_type(game, slot, Type::Grass)
}

fn holon_energy_wp_reduces_retreat(game: &GameState, slot: &PokemonSlot) -> bool {
    if slot.is_ex {
        return false;
    }
    if !has_named_energy(game, slot, "Holon Energy WP") {
        return false;
    }
    has_basic_energy_type(game, slot, Type::Psychic)
}

fn has_named_energy(game: &GameState, slot: &PokemonSlot, name: &str) -> bool {
    slot.attached_energy.iter().any(|energy| {
        game.card_meta
            .get(&energy.def_id)
            .map(|meta| meta.name == name)
            .unwrap_or(false)
    })
}

fn has_basic_energy_type(game: &GameState, slot: &PokemonSlot, energy_type: Type) -> bool {
    slot.attached_energy.iter().any(|energy| {
        game.card_meta
            .get(&energy.def_id)
            .map(|meta| {
                meta.energy_kind.as_deref() == Some("Basic") && meta.provides.contains(&energy_type)
            })
            .unwrap_or(false)
    })
}

pub fn holon_legacy_in_play(game: &GameState) -> bool {
    game.stadium_in_play()
        .map(|stadium| def_id_matches(&stadium.def_id, "DF", 74))
        .unwrap_or(false)
}

pub fn attack_overrides(
    game: &GameState,
    _attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    let Some(attacker) = game
        .current_player()
        .find_pokemon(attacker_id)
        .or_else(|| game.opponent_player().find_pokemon(attacker_id))
    else {
        return overrides;
    };
    let Some(defender) = game
        .opponent_player()
        .find_pokemon(defender_id)
        .or_else(|| game.current_player().find_pokemon(defender_id))
    else {
        return overrides;
    };

    if holon_legacy_in_play(game) && game.pokemon_is_delta(defender_id) {
        overrides.ignore_weakness = true;
    }
    if !defender.is_ex && has_holon_energy(game, defender, 84) && has_basic_energy_type(game, defender, Type::Fire)
    {
        overrides.ignore_weakness = true;
    }
    if !attacker.is_ex
        && has_holon_energy(game, attacker, 84)
        && has_basic_energy_type(game, attacker, Type::Fighting)
    {
        overrides.ignore_resistance = true;
    }
    if !defender.is_ex
        && attacker.is_ex
        && has_holon_energy(game, defender, 85)
        && has_basic_energy_type(game, defender, Type::Lightning)
    {
        overrides.damage_modifier -= 10;
    }
    if game.has_marker(attacker_id, "Delta Reduction") {
        overrides.pre_weakness_modifier -= 30;
    }
    if game.has_marker(attacker_id, "Charm") {
        overrides.pre_weakness_modifier -= 20;
    }
    if game.has_marker(defender_id, "Granite Head") {
        overrides.damage_modifier -= 10;
    }
    if game.has_marker(defender_id, "Protective Swirl") {
        overrides.ignore_weakness = true;
    }
    overrides
}

pub fn energy_provides_override(game: &GameState, card: &CardInstance) -> Option<Vec<Type>> {
    if def_id_matches(&card.def_id, "DF", 88) {
        if let Some(slot) = attached_slot_for_energy(game, card.id) {
            if slot.is_delta {
                return Some(all_energy_types());
            }
        }
        return Some(vec![Type::Colorless]);
    }
    if def_id_matches(&card.def_id, "DF", 89) {
        if let Some(slot) = attached_slot_for_energy(game, card.id) {
            if scramble_energy_active(game, slot) {
                return Some(all_energy_types());
            }
        }
        return Some(vec![Type::Colorless]);
    }
    if def_id_matches(&card.def_id, "DF", 87) {
        return Some(vec![Type::Colorless]);
    }
    None
}

pub fn on_energy_attached(
    game: &mut GameState,
    energy_id: CardInstanceId,
    target_id: CardInstanceId,
    _source: EnergyAttachmentSource,
) {
    let slot = match game
        .players
        .iter_mut()
        .find_map(|player| player.find_pokemon_mut(target_id))
    {
        Some(slot) => slot,
        None => return,
    };
    let energy_def_id = slot
        .attached_energy
        .iter()
        .find(|energy| energy.id == energy_id)
        .map(|energy| energy.def_id.clone());
    let Some(energy_def_id) = energy_def_id else {
        return;
    };
    if def_id_matches(&energy_def_id, "DF", 87) {
        if slot.stage == Stage::Basic {
            discard_attached_energy(game, target_id, energy_id);
            return;
        }
        let mut marker = Marker::new("Boost Energy");
        marker.source = Some(energy_id);
        marker.expires_after_turn = Some(game.turn.number);
        slot.markers.push(marker);
    }
    if def_id_matches(&energy_def_id, "DF", 89) {
        if slot.stage == Stage::Basic || slot.is_ex {
            discard_attached_energy(game, target_id, energy_id);
        }
    }
}

pub fn can_use_pokepower_override(
    game: &GameState,
    _player: PlayerId,
    pokemon_id: CardInstanceId,
) -> Option<bool> {
    if holon_legacy_in_play(game) && game.pokemon_is_delta(pokemon_id) {
        return Some(false);
    }
    None
}

pub fn discard_boost_energy(game: &mut GameState) {
    let mut to_discard: Vec<(CardInstanceId, CardInstanceId)> = Vec::new();
    for player in &mut game.players {
        if let Some(active) = player.active.as_mut() {
            for energy_id in collect_boost_from_slot(active) {
                to_discard.push((active.card.id, energy_id));
            }
        }
        for slot in &mut player.bench {
            for energy_id in collect_boost_from_slot(slot) {
                to_discard.push((slot.card.id, energy_id));
            }
        }
    }
    for (pokemon_id, energy_id) in to_discard {
        discard_attached_energy(game, pokemon_id, energy_id);
    }
}

fn has_holon_energy(game: &GameState, slot: &PokemonSlot, number: u32) -> bool {
    slot.attached_energy.iter().any(|energy| {
        def_id_matches(&energy.def_id, "DF", number)
            || game
                .card_meta
                .get(&energy.def_id)
                .map(|meta| {
                    meta.name
                        == format!(
                            "Holon Energy {}",
                            match number {
                                84 => "FF",
                                85 => "GL",
                                86 => "WP",
                                _ => "",
                            }
                        )
                })
                .unwrap_or(false)
    })
}

fn attached_slot_for_energy(game: &GameState, energy_id: CardInstanceId) -> Option<&PokemonSlot> {
    for player in &game.players {
        if let Some(active) = player.active.as_ref() {
            if active
                .attached_energy
                .iter()
                .any(|energy| energy.id == energy_id)
            {
                return Some(active);
            }
        }
        if let Some(slot) = player
            .bench
            .iter()
            .find(|slot| slot.attached_energy.iter().any(|energy| energy.id == energy_id))
        {
            return Some(slot);
        }
    }
    None
}

fn all_energy_types() -> Vec<Type> {
    vec![
        Type::Grass,
        Type::Fire,
        Type::Water,
        Type::Lightning,
        Type::Psychic,
        Type::Fighting,
        Type::Darkness,
        Type::Metal,
        Type::Colorless,
    ]
}

fn discard_attached_energy(game: &mut GameState, target_id: CardInstanceId, energy_id: CardInstanceId) {
    let owner = match crate::df::helpers::owner_for_source(game, target_id) {
        Some(player) => player,
        None => return,
    };
    let owner_index = match owner {
        PlayerId::P1 => 0,
        PlayerId::P2 => 1,
    };
    let slot = match game.players[owner_index].find_pokemon_mut(target_id) {
        Some(slot) => slot,
        None => return,
    };
    if let Some(index) = slot.attached_energy.iter().position(|energy| energy.id == energy_id) {
        let energy = slot.attached_energy.remove(index);
        game.players[owner_index].discard.add(energy);
    }
}

fn collect_boost_from_slot(slot: &mut PokemonSlot) -> Vec<CardInstanceId> {
    let mut to_discard = Vec::new();
    slot.markers.retain(|marker| {
        if marker.name == "Boost Energy" {
            if let Some(energy_id) = marker.source {
                to_discard.push(energy_id);
            }
            false
        } else {
            true
        }
    });
    to_discard
}
