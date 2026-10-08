# CardBench Magic single-card task: Mortipede

Implement Mortipede in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/mortipede.rs`. Keep its definition and activated ability in that candidate-owned module. Mortipede is a black `{3}{B}` 4/1 creature. Its `{2}{G}` ability gives itself “must be blocked if able” until end of turn.

The full-game trace pays for and resolves the ability before combat, observes an available Lurking Informant assigned as a blocker, confirms the two creatures exchange damage and die, and continues until seat zero wins. Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; task authority, scorer, materializer, and private task specification files are omitted. Do not add other files to the candidate submission.
