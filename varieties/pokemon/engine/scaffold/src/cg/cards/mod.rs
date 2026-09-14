//! Crystal Guardians remaining-PvP card modules (public stubs).
//! Remaining-PvP gold is private. This folder is the layout the set
//! task writes; `create_hooks("CG")` merges engine + these fragments.

use tcg_core::runtime_hooks::AttackOverrides;
use tcg_core::{Attack, CardInstanceId, GameState};

pub mod cg_003_attacks;
pub mod cg_013_attacks;
pub mod cg_020_attacks;
pub mod cg_022_attacks;
pub mod cg_024_attacks;
pub mod cg_025_attacks;
pub mod cg_027_attacks;
pub mod cg_028_attacks;
pub mod cg_036_attacks;
pub mod cg_037_attacks;
pub mod cg_089_attacks;
pub mod cg_091_attacks;
pub mod cg_092_attacks;
pub mod cg_093_attacks;
pub mod cg_094_attacks;
pub mod cg_095_kyogre_ex;
pub mod cg_096_attacks;
pub mod cg_097_attacks;
pub mod cg_098_attacks;
pub mod cg_099_attacks;
pub mod cg_100_attacks;

pub fn attack_overrides(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    overrides.merge(cg_003_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_013_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_020_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_022_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_024_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_025_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_027_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_028_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_036_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_037_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_089_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_091_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_092_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_093_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_094_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_095_kyogre_ex::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_096_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_097_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_098_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_099_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides.merge(cg_100_attacks::attack_overrides(game, attack, attacker_id, defender_id));
    overrides
}
