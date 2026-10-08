# CardBench Magic single-card task: Votary of the Conclave

Implement Votary of the Conclave in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/votary_of_the_conclave.rs` by providing `definition()` and `activated_ability()`.

Votary is a white 1/1 creature that costs `{W}`. Its ability is `{G}: Regenerate target creature`. The authority plays a shuffled 60-card game, casts Votary for exactly one white mana, and responds to Brainspoil targeting the untapped Votary by paying one green mana and targeting Votary with the regeneration ability. The trace checks the shield creation and replacement events, tapped/live post-shield state, Brainspoil resolution, and a terminal win. It also checks deterministic replay and a compiled wrong-color cost mutant.

This is diagnostic-only: one trace-passed RAV name from 291 unique names and 306 printings, zero cards credited, and zero reward. Change only the candidate module.
