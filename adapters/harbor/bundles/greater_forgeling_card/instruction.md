# CardBench Magic single-card task: Greater Forgeling

Implement Greater Forgeling in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/greater_forgeling.rs`. Keep its printed definition and activated self-pump ability in that candidate-owned module. Only this module is an allowed candidate edit.

Greater Forgeling is a red `{3}{R}{R}` 3/4 creature. Its `{1}{R}` activated ability gives itself +3/-3 until end of turn. The trace casts it, pays the exact activated cost, verifies its live 6/1 characteristics and cleanup restoration to 3/4, and then attacks for 6 in a deterministic full-game match to a seat-zero win.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; it omits task authority, the scorer, materializer, and task specification files. Do not add other files to the candidate submission.
