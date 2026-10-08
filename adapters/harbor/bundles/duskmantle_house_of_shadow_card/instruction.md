# CardBench Magic single-card task: Duskmantle, House of Shadow

Implement Duskmantle, House of Shadow in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/duskmantle_house_of_shadow.rs`. Keep its card definition and activated binding in that module. It is a colorless nonbasic land with “{T}: Target player mills a card.”

The full-game trace activates Duskmantle targeting the opponent, verifies that exactly one of their library objects moves to their graveyard, and checks the source paid its tap cost. Watchwolf then attacks to a game win. Deterministic replay and gameplay mutations cover mill count, tap cost, effect target, and player target requirement.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, scorer, materializer, and task specification files. Do not add other files to the candidate submission.
