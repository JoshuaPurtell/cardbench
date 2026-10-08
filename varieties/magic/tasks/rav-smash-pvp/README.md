# RAV Smash single-card task

Implement the candidate-owned Smash module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/smash.rs`. Smash is RAV collector #143, a red instant costing `{2}{R}`. Its ordered instructions destroy a target artifact and then draw one card.

The authority runs a legal shuffled sixty-card match. The opponent resolves a Boros Signet, the pilot casts Smash and pays three red mana, the trace records Smash destroying that exact Signet and moving it to its owner's graveyard, and a different card moves from the pilot's library to hand before Smash completes its graveyard transition. The pilot then wins through Viashino Slasher combat.

The authority validates the RAV catalog identity, source syntax, effect order, exact target and draw receipts, deterministic replay, and a semantic effect-order mutation. Diagnostic reference reports award no set coverage or Harbor reward. Independent review and a real candidate trial remain pending.

Run the materializer with `--workspace PATH --manifest PATH` and verify with the same arguments plus `--verify`. The Harbor authority bin is `rav-smash-pvp`; the local underscore-named Cargo target is `rav_smash_pvp`.
