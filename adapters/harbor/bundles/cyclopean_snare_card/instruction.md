# CardBench Magic single-card task: Cyclopean Snare

Implement Cyclopean Snare in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/cyclopean_snare.rs`. Keep its colorless `{2}` artifact definition and `{3}, {T}: tap target creature, return this source to its owner's hand` activated binding in that module.

The full-game trace casts Snare, activates it targeting the opponent's Watchwolf, verifies the `{3}` payment and source tap cost, confirms the exact target is tapped and the source returns to its owner's hand, then continues to a full-game win. Replay and gameplay mutations check each part of the behavior.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, scorer, materializer, and task specifications. Do not add other files to the candidate submission.
