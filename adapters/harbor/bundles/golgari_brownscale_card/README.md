# Golgari Brownscale task bundle

Implement the candidate-owned RAV #166 definition and graveyard-to-hand trigger. Its public identity is green `{1}{G}{G}`, Creature — Lizard, 2/3, Dredge 2, and a two-life gain on a graveyard-to-hand transition. The current `CardDefinition` model cannot assert the Lizard subtype; it is retained identity only.

The trusted authority checks the exact candidate AST before candidate calls, then validates a deterministic full-game sequence through typed session events: combat death, two-card Dredge, same-object return, source-bound trigger stack and resolution, two life gained, recast, and terminal game. Reference evidence is diagnostic only: zero card credit and zero Harbor reward.
