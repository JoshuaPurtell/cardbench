# CardBench Magic single-card task: Vigor Mortis

Implement Vigor Mortis in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/vigor_mortis.rs`. It is a black `{2}{B}{B}` sorcery. Return target creature card from your graveyard to the battlefield. If green mana was spent to cast it, put a +1/+1 counter on that creature.

The trusted authority uses ordinary shuffled 60-card duels. It checks legal combat death into the caster's graveyard, then verifies a green-generic payment returns the same creature with one counter. A second duel pays generic mana with black and verifies the returned creature has no counter. Each game continues to a seat-zero win and deterministic replay; cost, effect, and spent-color mutations are checked in-game.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; task authority, scorer, materializer, and private task specifications are omitted. Do not add other files to the candidate submission.
