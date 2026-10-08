# Wojek Apothecary RAV full-game task

Implement Wojek Apothecary (#36) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_apothecary.rs`. Keep its white `{2}{W}{W}` 1/1 creature definition and `{T}: Prevent the next 1 damage that would be dealt to target creature this turn and each other creature that shares a color with it` activation in that candidate-owned module.

The full shuffled PvP trace targets an opposing Watchwolf, creates one-point shields for that Watchwolf, a friendly Watchwolf, and Wojek Apothecary, and then blocks the attacker with the friendly Watchwolf. Each combat packet has one point prevented and two points dealt. The same game continues to a seat-zero win.

Only modify the declared candidate module. The private task authority, scorer, materializer, and test specification are not available in the public workspace.
