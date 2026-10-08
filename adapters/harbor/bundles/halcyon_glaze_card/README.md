# Halcyon Glaze card task

Implement Ravnica: City of Guilds #54 at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs`. Preserve the blue `{1}{U}{U}` Enchantment and its mandatory controller creature-spell trigger: it becomes a 4/4 blue Illusion Creature with Flying until end of turn while keeping its Enchantment type.

The isolated authority runs a normal shuffled 60-card duel and checks that the trigger resolves above each controlled creature spell, does not trigger for an opponent's creature spell, restores the Enchantment characteristics at Cleanup, and makes five exact four-damage flying attacks before a terminal Player 0 win. The report also enforces deterministic replay, gameplay mutations, source validation before candidate calls, and zero rewards/credit.
