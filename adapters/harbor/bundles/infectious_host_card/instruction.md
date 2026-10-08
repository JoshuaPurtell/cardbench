# CardBench Magic single-card task: Infectious Host

Implement Infectious Host in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/infectious_host.rs`, including both `definition()` and its candidate-owned `triggered_ability()`.

The card is a black 1/1 creature costing `{2}{B}`. Its mandatory dies trigger targets a player who loses 2 life. The verifier runs a legal full constructed game, destroys the Host with Putrefy, selects the opponent as the target, and checks the graveyard, source-incarnation, trigger, and life-loss events in order. The reference is diagnostic-only: 291 names, 306 printings, zero card credit, zero reward.
