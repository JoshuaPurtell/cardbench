# CardBench Magic single-card task: Vitu-Ghazi, the City-Tree

Implement Vitu-Ghazi, the City-Tree in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vitu_ghazi.rs`. Keep all target-card mechanics in this module. Only this module is an allowed candidate edit.

The candidate-owned module must declare the RAV land identity, its tap-for-colorless mana ability, and its `{2}{G}{W}` tap ability that creates one green 1/1 Saproling creature token. The trace taps one City-Tree to produce a colorless mana that is actually spent on a generic fixture-spell cost, then untaps that same land and activates the token ability. It checks exact mana and tap costs, ordered typed events, live token characteristics, later combat, deterministic replay, and compiled semantic mutants.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, the scorer, materializer, and task specification files. Do not add other files to the candidate submission.
