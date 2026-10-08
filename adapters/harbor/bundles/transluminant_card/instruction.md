# CardBench Magic single-card task: Transluminant

Implement the RAV Transluminant definition and its candidate-owned `triggered_ability()` in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/transluminant.rs`.

Transluminant is a green 2/2 creature costing `{1}{G}`. The optional dies ability creates one 1/1 white Spirit creature token with flying. The verifier casts it from a legal 60-card Selesnya deck and uses a legal Golgari Putrefy to destroy it. Typed events must show the source incarnation entering the graveyard before its trigger resolves.

Two full-game traces cover the choice: accepting creates a live white Spirit 1/1 with flying; declining creates no token from this trigger. Both traces must replay deterministically. The pilot is diagnostic-only against 291 unique RAV names and 306 printings and awards no card credit or reward.
