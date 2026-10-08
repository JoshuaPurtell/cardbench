# CardBench Magic single-card task: Ghosts of the Innocent

Implement Ghosts of the Innocent (#20) in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/ghosts_of_the_innocent.rs`. It is a white `{5}{W}{W}` 4/5 creature. If a source would deal damage to a permanent or player, it deals half that damage, rounded down, to that permanent or player instead.

The trusted authority casts Ghosts in a normal shuffled duel, then observes an opposing Char's four-damage creature packet and its two-damage self-packet being halved. A second Char targets the player. The game also records halved Watchwolf combat damage and continues to a seat-zero win. The report checks deterministic replay and in-game mutations to the cost, characteristics, and damage-replacement binding.

Read the task README and `features.toml` in the trusted handoff before editing. The public workspace exposes only the target module; task authority, scorer, materializer, and private task specifications are omitted. Do not add other files to the candidate submission.
