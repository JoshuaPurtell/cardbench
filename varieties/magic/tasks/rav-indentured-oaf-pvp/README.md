# Indentured Oaf RAV card task

Implement Ravnica: City of Guilds #133, Indentured Oaf: a red `{3}{R}` 4/3 creature. Its printed text is “Prevent all damage that Indentured Oaf would deal to red creatures.” The candidate module exposes the card definition only.

The authority uses shuffled 60-card decks. Seat zero casts Oaf; seat one casts a red-white Boros Recruit, choosing red for its hybrid cost, and blocks Oaf once. Recruit's First Strike deals one damage to Oaf in the first combat-damage step. Oaf's four damage to Recruit is prevented in the normal combat-damage step, leaving Recruit undamaged. Oaf then attacks unblocked until seat zero wins. This distinguishes source-side prevention to red creatures from red-source prevention to Oaf. The source incarnation remains on the battlefield through the measured combat.

The original RAV wording names Indentured Oaf. Current Oracle substitutes “this creature” without changing the effect. The printed type line is Creature — Ogre Warrior, which is retained in task metadata; the current `CardDefinition` has no subtype field, so the subtype is not an executable assertion.

The reference trace must replay deterministically and detect mutations to the mana cost, power, lethal toughness, missing prevention, wrong prevention color, and reversed prevention direction. The source guard checks the exact normalized module and its restricted AST before calling the candidate factory, and rejects a compile-valid side-effect injection first.

This is a local diagnostic task. All rewards and card credits remain zero.
