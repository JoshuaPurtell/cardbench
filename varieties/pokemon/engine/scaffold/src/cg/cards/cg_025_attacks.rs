//! Crystal Guardians remaining-PvP card module (public stub).
//! Remaining-PvP gold stays private in CardCodeBench.

use tcg_core::runtime_hooks::{AttackOverrides, RuntimeHooks};
use tcg_core::{Attack, CardInstanceId, GameState};

pub fn attacks() -> Vec<Attack> {
    Vec::new()
}

pub fn hooks() -> RuntimeHooks {
    RuntimeHooks::empty()
}

pub fn attack_overrides(
    _game: &GameState,
    _attack: &Attack,
    _attacker_id: CardInstanceId,
    _defender_id: CardInstanceId,
) -> AttackOverrides {
    AttackOverrides::default()
}
