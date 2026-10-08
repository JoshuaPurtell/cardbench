# Barbarian Riftcutter card task

Implement Barbarian Riftcutter (#114) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/barbarian_riftcutter.rs` by providing its candidate-owned `definition()` and `activated_ability()`.

Barbarian Riftcutter is a red 3/3 creature that costs `{4}{R}` and has “`{R}`, sacrifice this creature: destroy target land.” The deterministic full-game reference casts the creature with an explicit five-mana payment, confirms its live characteristics and attack damage, then pays red and sacrifices it to destroy an opposing land.

The authority checks ordered typed payment, casting, resolution, entry, combat, activation-cost, sacrifice, destruction, resolution, and game-end events; replays the shuffled game deterministically; and rejects compiled cost, sacrifice, and effect mutants. The diagnostic report covers 1 of 291 RAV card names from 306 printings, credits no cards, and awards zero Harbor reward. No provider trial is part of this task.

The task workspace exposes only the candidate card module. Run `tests/test.sh` from the provided workspace when evaluating a submission.
