# Wojek Embermage card task

Implement Wojek Embermage (#152) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_embermage.rs`. Keep its red `{3}{R}` 1/2 creature definition and `{T}` activated ability in that candidate-owned module.

When the ability resolves, it deals exactly 1 damage to its target creature and every other creature on the battlefield that shares at least one color with that target at resolution time. The full shuffled PvP trace targets an opposing Watchwolf, damages a friendly Watchwolf with the same colors, leaves the red source itself outside the recipient set, then continues through combat to a full-game win.

The private authority checks identity and registry delegation, exact cost payment, the activation tap cost and target, Radiance recipient scope, ordered damage and resolution events, deterministic replay, and gameplay mutations. This is a diagnostic task with no card credit. The public workspace exposes only the target module; do not add other candidate files.
