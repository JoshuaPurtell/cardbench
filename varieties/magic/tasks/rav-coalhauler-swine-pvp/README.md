# Coalhauler Swine card task

Implement the candidate-owned definition and received-damage trigger in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs`. Preserve RAV #119 identity, the red `{4}{R}{R}` creature cost, 4/4 body, and the mandatory trigger that makes Coalhauler Swine deal the amount of damage it received to each player.

The trusted authority uses shuffled, legal 60-card decks in a two-player match. It casts Coalhauler Swine and Viashino Fangtail, has Fangtail deal one damage to the Swine, and verifies the exact trigger source/incarnation and one damage packet to each surviving player. It checks both players at 19 life immediately after resolution, continues the match to a seat-zero win, and repeats the same seed for deterministic replay. Five gameplay mutations must be detected.

This is a local diagnostic task. It awards no cards or reward and does not run Harbor/provider trials during reference qualification.
