# RAV Searing Meditation single-card diagnostic

This task targets Searing Meditation (RAV collector #226), a red-white enchantment costing `{2}{R}{W}`. In a legal shuffled 60-card PvP game, Lightning Helix gains three life and causes Searing Meditation's trigger to be put on the stack. The controller may pay `{1}{R}{W}` as the trigger resolves; paying chooses the opponent as a legal target and deals exactly two damage. A paired trace declines the optional payment and produces no Searing Meditation damage.

Both branches reach a terminal player-zero win and replay deterministically. The authority also removes the trigger and changes its damage amount in game-level mutation controls. Reports keep `pilot_reward`, `harbor_reward`, and `cards_credited` at zero. This diagnostic does not complete set coverage or grant qualification credit.

The target is `src/cards/searing_meditation.rs`; its candidate-owned API consists of `definition()` and `triggered_ability()`. The authority checks the printed `{2}{R}{W}` card cost, the optional `{1}{R}{W}` trigger cost, the life-gain condition, and its two-damage any-target effect through the public event projection. The task bundle's compile-valid stub uses one damage and must be rejected.

The rules baseline is the Wizards Comprehensive Rules effective August 7, 2026, with an authority cutoff of September 23, 2026. A real candidate trial and independent review remain separate qualification steps.
