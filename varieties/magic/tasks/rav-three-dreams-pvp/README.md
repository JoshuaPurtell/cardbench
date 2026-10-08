# Three Dreams full-game card task

Implement the candidate-owned RAV Three Dreams definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/three_dreams.rs`. Its controller may privately search for up to three Aura cards with different names, reveal selected cards, put them into hand, and shuffle.

The isolated authority must run shuffled 60-card PvP games that select three differently named Auras and that select none while legal candidates remain. Each game continues through combat to a terminal result and checks ordered payment, cast, private search, reveals, hand transfers, shuffle, and game-end events. It must check deterministic replay and reject mutations that reduce the maximum, allow duplicate names, or omit selected-card reveals. This diagnostic reports one identity among 291 RAV names and 306 printings with zero card credit and zero reward.
