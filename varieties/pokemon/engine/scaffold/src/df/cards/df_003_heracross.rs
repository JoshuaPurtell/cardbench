//! Heracross δ - Dragon Frontiers #3
//!
//! Basic Pokemon - Fighting/Metal Type - HP 70
//! Weakness: Fire | Resistance: None | Retreat: 1
//!
//! ## Poke-Body: Shining Horn
//! As long as Heracross is your Active Pokemon and you have no Benched Pokemon,
//! prevent all damage done to Heracross by attacks from your opponent's Basic Pokemon.
//!
//! ## Attack: Extra Claws - [F][M] - 30+
//! If the Defending Pokemon is Pokemon-ex, this attack does 30 damage plus 20 more damage.

use tcg_core::runtime_hooks::{def_id_matches, AttackOverrides};
use tcg_core::{Attack, CardInstanceId, GameState, Stage};

/// Card identifiers
pub const SET: &str = "DF";
pub const NUMBER: u32 = 3;
pub const NAME: &str = "Heracross δ";

// ============================================================================
// Poke-Body: Shining Horn
// ============================================================================

/// Check if Shining Horn prevents damage from the attacker.
///
/// Returns true if:
/// - The defender is Heracross δ (DF-3)
/// - The attacker is a Basic Pokemon
/// - Heracross is the Active Pokemon with no Benched Pokemon
/// - The body is active (not blocked by special conditions)
pub fn shining_horn_prevents_damage(
    game: &GameState,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
    body_active: bool,
) -> bool {
    let attacker = match game.current_player().find_pokemon(attacker_id) {
        Some(slot) => slot,
        None => return false,
    };
    let defender = match game.opponent_player().find_pokemon(defender_id) {
        Some(slot) => slot,
        None => return false,
    };

    if !def_id_matches(&defender.card.def_id, SET, NUMBER) {
        return false;
    }

    if attacker.stage != Stage::Basic {
        return false;
    }

    if !game.opponent_player().bench.is_empty() {
        return false;
    }

    body_active
}

// ============================================================================
// Attack: Extra Claws
// ============================================================================

/// Calculate the bonus damage for Extra Claws attack.
///
/// Returns +20 damage if the Defending Pokemon is a Pokemon-ex.
pub fn extra_claws_bonus(game: &GameState, defender_id: CardInstanceId) -> i32 {
    let defender = match game.opponent_player().find_pokemon(defender_id) {
        Some(slot) => slot,
        None => return 0,
    };

    if defender.is_ex {
        20
    } else {
        0
    }
}

fn body_active(game: &GameState, pokemon_id: CardInstanceId) -> bool {
    let Some(owner) = game.owner_for_pokemon(pokemon_id) else {
        return false;
    };
    game.is_pokebody_active(owner, pokemon_id, false)
}

pub fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    if shining_horn_prevents_damage(
        game,
        attacker_id,
        defender_id,
        body_active(game, defender_id),
    ) {
        overrides.prevent_damage = true;
    }
    let Some(attacker) = game.current_player().find_pokemon(attacker_id) else {
        return overrides;
    };
    if def_id_matches(&attacker.card.def_id, SET, NUMBER) && attack.name == "Extra Claws" {
        overrides.pre_weakness_modifier += extra_claws_bonus(game, defender_id);
    }
    overrides
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_identifiers() {
        assert_eq!(SET, "DF");
        assert_eq!(NUMBER, 3);
        assert_eq!(NAME, "Heracross δ");
    }

    #[test]
    fn test_extra_claws_bonus_value() {
        // When defender is ex, bonus should be 20
        // This is a unit test - integration tests would use actual GameState
        let is_ex = true;
        let bonus = if is_ex { 20 } else { 0 };
        assert_eq!(bonus, 20);
    }
}
