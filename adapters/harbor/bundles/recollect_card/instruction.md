# CardBench Magic single-card task: Recollect

Implement Recollect in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/recollect.rs`. Keep its printed definition in that candidate-owned module. It is a green `{2}{G}` sorcery that returns target card from your graveyard to your hand.

The trace puts an instant in the pilot's graveyard, targets that exact own-graveyard object, verifies its move to the pilot's hand during Recollect's resolution, and continues a full PvP game to a win. It checks deterministic replay and mutations to target scope, target player, cost, and color.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, the scorer, materializer, and task specification files. Do not add other files to the candidate submission.
