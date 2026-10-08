# Implement War-Torch Goblin

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/war_torch_goblin.rs` as a literal-only module exporting `definition()` and `sacrifice_deal_two_to_blocker_ability()`. Preserve the RAV #151 identity, `{R}` cost, red 1/1 creature characteristics, and candidate-owned `{R}`, sacrifice-source activated binding targeting a blocking creature and dealing exactly 2 damage.

The reference authority runs an ordinary shuffled two-player game. It casts War-Torch Goblin, casts Goblin Fire Fiend, records Terraformer blocking Fire Fiend, pays red mana and sacrifices War-Torch Goblin to kill the blocker before combat damage. Fire Fiend attacks later and the game reaches a seat-zero win. Focused mutations test the printed cost and the ability's color, tap, sacrifice, target, and damage semantics. Reference reports award no RAV coverage or Harbor reward.
