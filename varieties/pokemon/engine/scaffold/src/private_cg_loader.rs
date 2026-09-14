//! Public shim for the package-time Crystal Guardians module.
//!
//! The referenced directory is gitignored and exists only when `cg_private` is enabled.

#[rustfmt::skip]
#[path = "cg_private/mod.rs"]
mod installed;

/// Build the private per-card table while retaining the public set-wide rules.
pub fn create() -> tcg_core::runtime_hooks::RuntimeHooks {
    let mut hooks = installed::create();
    hooks.is_double_rainbow = crate::cg::engine::is_double_rainbow;
    hooks.prevents_attack_effects = crate::cg::engine::prevents_attack_effects;
    hooks.tool_discard_timing_override = crate::cg::engine::tool_discard_timing_override;
    hooks.energy_units = crate::cg::engine::energy_units;
    hooks
}
