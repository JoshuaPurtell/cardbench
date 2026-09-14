//! Nidorino δ - Dragon Frontiers #35
//!
//! Stage 1 Pokemon - Darkness Type - HP 70
//! Evolves from: Nidoran ♂
//! Weakness: Psychic | Resistance: None | Retreat: 1
//!
//! ## Attacks
//! [DC] Rage - 10+ damage. Does 10 damage plus 10 more damage for each damage
//! counter on Nidorino.
//!
//! [CCC] Horn Drill - 30 damage.

/// Card identifiers
pub const SET: &str = "DF";
pub const NUMBER: u32 = 35;
pub const NAME: &str = "Nidorino δ";


use tcg_core::runtime_hooks::{def_id_matches, AttackOverrides};
use tcg_core::{Attack, CardInstanceId, GameState};

pub fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    _defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    let Some(attacker) = game.current_player().find_pokemon(attacker_id) else {
        return overrides;
    };
    if def_id_matches(&attacker.card.def_id, SET, NUMBER) && attack.name == "Rage" {
        overrides.pre_weakness_modifier += attacker.damage_counters as i32 * 10;
    }
    overrides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_identifiers() {
        assert_eq!(SET, "DF");
        assert_eq!(NUMBER, 35);
        assert_eq!(NAME, "Nidorino δ");
    }
}
