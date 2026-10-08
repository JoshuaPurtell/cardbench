# CardBench Magic single-card task: Golgari Germination

Implement Golgari Germination in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_germination.rs`. Keep its definition and triggered ability in that candidate-owned module. It is a black-green `{1}{B}{G}` enchantment. Whenever a nontoken creature you control dies, it creates one green 1/1 Saproling creature token.

The full-game trace casts the enchantment, attacks with a controlled nontoken creature into an available blocker, and follows the dies trigger through the stack to token creation. The legal shuffled duel continues to a seat-zero win. Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; task authority, scorer, materializer, and private task specification files are omitted. Do not add other files to the candidate submission.
