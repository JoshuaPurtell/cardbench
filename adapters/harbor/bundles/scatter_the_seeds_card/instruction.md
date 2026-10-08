# RAV Scatter the Seeds (#181)

Implement the candidate-owned `CardDefinition` for the green Instant Scatter
the Seeds. Its cost is exactly `{3}{G}{G}`, it has Convoke, and it creates three
green 1/1 Saproling creature tokens. Preserve the module's public shape and do
not edit other files.

The isolated full-game diagnostic pays three green mana and taps two distinct
creatures for the three generic mana through Convoke, resolves the spell, checks
the three live token objects, then attacks with those tokens until the opponent
loses. It replays deterministically and runs compiled semantic mutations. The
result is diagnostic-only, with zero card credit and zero reward.
