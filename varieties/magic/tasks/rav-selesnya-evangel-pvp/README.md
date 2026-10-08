# RAV Selesnya Evangel single-card PvP task

This task covers the Ravnica: City of Guilds printing of Selesnya Evangel,
collector #228: a green-white 1/2 creature with an activated ability that taps
its source and another creature, pays one generic mana, and creates a green 1/1
Saproling. The card module keeps only the public identity and a CardBench-authored
semantic operation; it does not reproduce the card's Oracle prose.

The reference trace uses a legal shuffled 60-card green-white deck against a
legal 60-card basic-land deck. It casts Selesnya Evangel through the normal
spell lifecycle and verifies one green and one white mana were paid. After the
creature can legally pay its tap symbol, the trace activates the bound ability,
records payment and both creature tap costs, puts the ability on the stack, and
checks ordered resolution and token-creation events. The Saproling remains live
on the battlefield as a green 1/1 creature with the Saproling subtype. The game
continues to a terminal seat-zero win and the seeded match is replayed.

The source gate accepts only direct literal identity and ability definitions.
Semantic omissions are admitted far enough to reach the game trace. Four
mutations prove the oracle responds to removing the source tap cost, the
additional-creature tap cost, the token creation effect, and the white cast
symbol. The Harbor materializer supplies a
compile-valid ability stub with no effect; the trusted trace must reject it for
failing to create the Saproling.

The task uses the Comprehensive Rules effective 2026-08-07 and the RAV printing
contract. Relevant rules include colored mana payment, ordinary casting and
resolution, priority to activate, activated-ability costs and stack resolution,
creature tap-symbol restrictions, and token characteristics. This one-card
trace is diagnostic over the full RAV denominator of 291 unique identities and
306 printings. It does not grant set coverage or Harbor reward.

Materialize with `materialize_selesnya_evangel_task.py --workspace PATH
--manifest PATH`, then verify with the same arguments plus `--verify`.
`score_selesnya_evangel_pvp.py` runs the candidate in the networkless bounded
worker. No provider-backed candidate trial is part of this task.
