Implement RAV Elvish Skysweeper (#162) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/elvish_skysweeper.rs`.

The card is a green `{G}` 1/1 Creature. Its activated ability costs `{4}{G}` and the sacrifice of one creature, targets a creature with flying, and destroys that target. Keep the ability source on the battlefield; it does not have a tap cost. Implement both the printed definition and the activated-ability binding in this module.

The authority first checks the candidate module's pinned, declarative AST and rejects a compile-valid injected side effect before invoking candidate code. It then checks your module against a shuffled two-player 60-card match, exact object identities and event order, replay, and semantic mutations. The candidate workspace permits edits only to the declared card module. Do not change task metadata, the scorer, the authority, or shared registry files.
