# Ivy Dancer RAV full-game task

Implement Ivy Dancer (#171) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ivy_dancer.rs`. Its printed definition is a green `{2}{G}` 1/2 creature. Its `{T}` activated ability targets a creature and gives it Forestwalk until end of turn.

The deterministic shuffled 60-card duel casts Ivy Dancer, pays the printed cost, and activates its tap ability targeting a Watchwolf. The authority observes Forestwalk on the target while the defender controls a Forest, confirms the engine rejects an illegal block, then verifies the granted keyword expires by the next turn. The legal game continues to a seat-zero win. The authority also tests gameplay mutations to the effect, activation cost, casting cost, power, and toughness.

This is an uncredited local diagnostic. It runs no provider trial and awards no card credit, set completion, or Harbor reward.
