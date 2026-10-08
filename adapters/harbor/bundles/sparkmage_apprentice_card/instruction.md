# Implement Sparkmage Apprentice

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sparkmage_apprentice.rs` as a literal-only module containing `definition()` and `triggered_ability_binding()`. Preserve RAV #144 identity, the `{1}{R}` red 1/1 creature characteristics, and the mandatory `etb-deal-one` trigger targeting a player or creature and dealing exactly one damage.

The reference casts Sparkmage Apprentice from an ordinary shuffled two-player game, selects the opponent's Elves of Deep Shadow for its trigger, resolves one damage, and records the target's lethal-damage state-based action and graveyard move. Seat zero then attacks through the empty blocker lane and wins the same game. The report credits no RAV coverage or Harbor reward.
