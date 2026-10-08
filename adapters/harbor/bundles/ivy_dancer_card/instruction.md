# CardBench Magic single-card task: Ivy Dancer

Implement Ivy Dancer in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ivy_dancer.rs`. Keep its definition and activated ability in that candidate-owned module. Ivy Dancer is a green `{2}{G}` 1/2 creature. Its `{T}` ability targets a creature and gives it Forestwalk until end of turn.

The full-game trace pays for and resolves the ability on a Watchwolf. With the defender controlling a Forest, the authority checks the Forestwalk keyword, checks that the engine rejects an attempted illegal block, and observes the effect expire the next turn. The legal shuffled duel then reaches a seat-zero win. Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; task authority, scorer, materializer, and private task specification files are omitted. Do not add other files to the candidate submission.
