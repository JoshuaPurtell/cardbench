# Hour of Reckoning PvP task

Implement the candidate-owned Hour of Reckoning definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/hour_of_reckoning.rs`.

## Source note

This task follows [`RAV_PVP_SOURCE_CONTRACT.md`](../rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md): the RAV printing controls public identity and gameplay semantics; current Oracle wording is consulted only to interpret rules questions. The public [RAV #21 record](https://mtg.wtf/card/plst/RAV-21/Hour-of-Reckoning) confirms a white `{4}{W}{W}{W}` Sorcery with Convoke that destroys all nontoken creatures. The current wording has no gameplay-changing difference from the RAV behavior represented here. No image, scan, flavor text, or source text corpus is retained.

The reference authority runs a deterministic, legal 60-card duel. It casts Hour of Reckoning with Convoke, verifies the exact mana and creature contributions, destroys every nontoken creature on both battlefields, leaves creature tokens on the battlefield, and continues the game through a terminal result. It checks ordered event receipts, deterministic replay, and semantic mutations to the cost, Convoke keyword, and destruction effect. The candidate module is parsed and checked against an exact AST/source contract before candidate functions run; a compile-valid injected filesystem side effect must be rejected.

This is an uncredited diagnostic against the 291-name / 306-printing RAV catalog. It reports zero card credit and zero reward. Candidate trials and provider-backed runs are outside this task.
