# Glimpse the Unthinkable RAV full-game task

Implement Glimpse the Unthinkable (#208) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/glimpse_the_unthinkable.rs`. Keep its candidate-owned definition in this module. It is a blue-black `{U}{B}` sorcery that targets a player and mills exactly ten cards.

The deterministic shuffled 60-card PvP trace pays the exact two colored mana, retains PlayerId(1) as the chosen target, and observes ten distinct objects leave that player's library for their graveyard. It checks the library's before/after counts and event order through Glimpse's own move from the stack to its owner's graveyard. A supporting Moroii finishes the full game through repeated combat, so the trace covers a legal match beyond the targeted mill.

The authority checks the declarative candidate-module boundary, registered-definition delegation, deterministic replay, and in-game mutations for nine or eleven cards milled, wrong target, wrong casting cost, and wrong color identity. This is a diagnostic task; it awards no card credit or Harbor reward.
