# CardBench Magic single-card task: Infectious Host

Implement Infectious Host's definition and candidate-owned `triggered_ability()` in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/infectious_host.rs`.

Infectious Host is a black 1/1 creature costing `{2}{B}`. When it dies, its mandatory ability targets a player who loses 2 life. The verifier casts it from a legal 60-card Golgari deck, then uses Putrefy in a legal constructed game to destroy it. The typed trace proves the Host's graveyard move precedes target selection and trigger resolution, and records the chosen opponent losing exactly two life.

The reference task is diagnostic-only against 291 distinct RAV names and 306 printings. It awards no card credit or reward; independent review and a candidate trial are separate gates.
