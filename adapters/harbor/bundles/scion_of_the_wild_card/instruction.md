# CardBench Magic single-card task: Scion of the Wild

Implement Scion of the Wild in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/scion_of_the_wild.rs` by providing `definition()` and `static_continuous_effect()`.

Scion is a green creature that costs `{1}{G}{G}`. Its printed base power and toughness are 0/0, and its power and toughness each equal the number of creatures you control.

The authority plays a legal shuffled full game. It pays three green mana, casts Scion before any other creature, checks its live 1/1 characteristics, casts Watchwolf, checks Scion updates to 2/2, then attacks and finishes the game. It validates typed ordered payment, cast, resolution, entry, combat, and end events; deterministic replay; and an in-game compiled mutant with the continuous effect removed.

The task is diagnostic-only: one trace-passed name from 291 unique RAV names / 306 printings, zero credited cards, and zero reward. Change only the candidate module.
