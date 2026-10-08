Implement the candidate-owned Fists of Ironwood module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/fists_of_ironwood.rs`.

Fists of Ironwood is a green `{1}{G}` Enchantment Aura that enchants a Creature and grants that creature Trample. Its mandatory enter-the-battlefield trigger creates exactly two green 1/1 Saproling creature tokens.

Keep the module declarative and limited to the card definition and trigger binding. The isolated authority checks the exact Aura and Watchwolf objects, trigger incarnation, both token objects and characteristics, a real block, lethal damage to the blocker, two excess Trample damage to the defending player, the terminal duel, deterministic replay, and semantic mutants. Do not add rules prose, art, scans, or flavor text.
