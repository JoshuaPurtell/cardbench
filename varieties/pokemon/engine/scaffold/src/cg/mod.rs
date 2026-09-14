pub mod all_cards;
pub mod engine;
pub mod hooks;
pub mod cards;

#[cfg(feature = "cg_private")]
pub use crate::cg_private::create;
#[cfg(not(feature = "cg_private"))]
pub use hooks::create;

pub use all_cards as runtime;
pub use all_cards::{attack_effect_ast, power_effect_ast, trainer_effect_ast, CG_POWERS, CG_TRAINERS};
