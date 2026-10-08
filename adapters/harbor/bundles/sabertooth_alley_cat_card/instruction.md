# Implement Sabertooth Alley Cat

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sabertooth_alley_cat.rs` as a literal-only module exposing `definition()`. Preserve the RAV #140 identity, `{1}{R}{R}` red creature characteristics, 2/1 power and toughness, and the rule that it cannot block unless its controller controls a Mountain.

The shuffled PvP reference casts the Cat before its controller has a Mountain, checks that an attempted block is rejected atomically, then plays a Mountain and blocks a War-Torch Goblin. The Cat and Goblin die to combat damage, the duel reaches a terminal seat-zero win, and the same seed replays identically. Mutations removing the restriction, changing the cost, or changing power must fail.
