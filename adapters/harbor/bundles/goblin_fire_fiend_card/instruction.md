# Implement Goblin Fire Fiend

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_fire_fiend.rs` as a literal-only module exporting `definition()` and `pump_plus_one_power_ability()`. Preserve RAV #127, `{3}{R}`, the red 1/1 creature, Haste, must-be-blocked-if-able behavior, and candidate-owned `{R}` activated ability that gives its source +1/+0 until end of turn.

The reference authority plays an ordinary shuffled two-player game. It casts and pays for Fire Fiend, attacks it during the same turn, has a live Lurking Informant block, pays and resolves the red pump ability, and verifies the Fire Fiend deals 2 combat damage to kill the 1/2 blocker before both creatures die. A later attack and damage event completes the same game. Mutations test Haste, the blocking requirement, pump amount, activation color, casting cost, and base power. Diagnostic reports award no RAV coverage or Harbor reward.
