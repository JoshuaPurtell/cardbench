# CardBench Magic single-card task: Shadow of Doubt

Implement the RAV Shadow of Doubt module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/shadow_of_doubt.rs`.

Shadow of Doubt is a blue-black instant with cost `{U/B}{U/B}`. Its first resolving effect prevents every player from searching libraries for the rest of the current turn. Its second effect draws one card for its controller. Preserve that order and the exact hybrid cost.

The trusted authority runs a shuffled 60-card PvP game. It casts Shadow in response to Farseek, checks the prevented same-turn search and the one-card draw, then verifies a later-turn search can privately find a Mountain and shuffle. The full game continues through combat to a terminal win and must replay deterministically. The task awards no RAV set credit or Harbor reward.
