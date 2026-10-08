# RAV Skyknight Legionnaire single-card PvP pilot

This task covers the Ravnica: City of Guilds printing of Skyknight Legionnaire,
collector #232: a red-white 2/2 creature with Flying and Haste, printed mana
cost `{1}{R}{W}`. The source is a pure declarative `definition() ->
CardDefinition` literal in `src/cards/skyknight_legionnaire.rs`.

The reference trace plays a shuffled 60-card Boros basic-land/Skyknight deck
against a legal 60-card Boros Aggro deck controlled by a pass-only policy. It
casts Skyknight Legionnaire in precombat main, pays red, white, and one generic
mana, resolves the creature, and declares that same permanent as an attacker
in the same turn. The structured event report binds the attacker object to the
spell cast, resolution, battlefield move, and `AttackersDeclared` receipt. The
trace checks live 2/2 characteristics and Flying/Haste, then repeats the match
with the same seed.

The trusted syntax gate accepts only the card's declarative fields, exact card
frame, exact red and white colored symbols, and Flying/Haste keyword names.
It permits semantic omissions and value changes so the full-game checks catch
incorrect power, toughness, mana cost, Flying, and Haste. In particular,
removing Haste prevents the new creature from attacking on its entry turn and
fails the same-turn combat check.

The task uses the Comprehensive Rules effective 2026-08-07 and the RAV printing
identity/characteristic contract. It contains no card image, flavor text, or
reproduced Oracle text. This is a one-card diagnostic against the complete RAV
denominator of 291 unique names and 306 printings; it grants no set coverage or
Harbor reward. A real candidate trial and independent review are still needed.

Materialize with `materialize_skyknight_legionnaire_task.py --workspace PATH
--manifest PATH`, then verify the sealed workspace with the same arguments plus
`--verify`. `score_skyknight_legionnaire_pvp.py` runs the candidate in the
networkless bounded worker using the immutable worker image configured for
Magic tasks.
