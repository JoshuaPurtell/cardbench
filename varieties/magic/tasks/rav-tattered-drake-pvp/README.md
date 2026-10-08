# Tattered Drake card task

Implement Tattered Drake (#68) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/tattered_drake.rs` by providing the candidate-owned `definition()` and `activated_ability()` functions.

Tattered Drake is a blue 2/2 creature with Flying that costs `{4}{U}`. Its activated ability is `{B}: Regenerate Tattered Drake`. The deterministic full-game trace pays the printed spell cost, answers an opponent's Char with the no-target self-regeneration ability, and verifies that the shield saves Drake from lethal damage, taps it, and leaves it alive. Drake later attacks through a ground Elves of Deep Shadow. A rejected block probe verifies that live Flying characteristics prevent the Elf from blocking; the accepted game then submits a legal empty blocker declaration and reaches a seat-zero win.

The authority validates ordered typed payment, cast, resolution, entry, activation, shield, attack, block, combat-damage, and end events; live creature characteristics and block legality; deterministic replay; a wrong-color regeneration mutant; and a compiled Flying-removal mutant. Its diagnostic report is 1 of 291 unique RAV names from 306 printings, with zero cards credited and zero reward. This is not a provider trial.
