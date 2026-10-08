# Vinelasher Kudzu card task

Implement the RAV Vinelasher Kudzu module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vinelasher_kudzu.rs`. Preserve its printed `{1}{G}` green 1/1 Creature — Plant identity and mandatory controller-land-entry ability that puts one +1/+1 counter on Vinelasher Kudzu.

The reference authority casts Kudzu in a shuffled 60-card duel, then observes both a controller land entry and an opponent land entry. Each controller land after Kudzu's entry must put one ordinary trigger on the stack and one counter on the same source after resolution; an opponent's land must not trigger it. The game finishes with a Player 0 terminal win and deterministic replay. The current engine `CardDefinition` has no subtype field, so Plant is retained in metadata but not asserted by runtime state.
