# Wojek Apothecary card task

Implement Wojek Apothecary (#36) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_apothecary.rs`. Keep the white `{2}{W}{W}` 1/1 creature definition and `{T}` activated ability candidate-owned in that module.

The ability targets a creature and creates an independent one-point damage-prevention shield for that creature and every other current creature that shares at least one color with it. The full shuffled PvP trace targets an opposing Watchwolf, shields the friendly Watchwolf and Wojek Apothecary as well, then blocks the attacker. It verifies that one point is prevented from each creature's combat damage and that the remaining two damage is dealt before the same game reaches a seat-zero win.

The private authority checks identity and registry delegation, exact casting payment, tap cost, target, exact Radiance scope, prevention and reduced combat damage, deterministic replay, and semantic mutations. This diagnostic task awards no card credit. The public workspace exposes only the target module.
