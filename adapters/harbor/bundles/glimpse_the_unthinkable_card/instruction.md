# CardBench Magic single-card task: Glimpse the Unthinkable

Implement Glimpse the Unthinkable in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/glimpse_the_unthinkable.rs`. Keep its printed definition in that candidate-owned module. It is a blue-black `{U}{B}` sorcery that targets a player and mills exactly ten cards.

The full-game trace verifies the exact cost, target, ten distinct library-to-graveyard moves, before/after library size, spell-resolution and terminal-zone order, deterministic replay, and a complete PvP game. It also runs gameplay mutations for mill count, target, cost, and color identity.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, the scorer, materializer, and task specification files. Do not add other files to the candidate submission.
