Implement the candidate-owned RAV Roofstalker Wight module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/roofstalker_wight.rs`.

Keep it declarative. It owns the black 2/1 creature definition with printed cost `{1}{B}` and the `{1}{U}` activated ability that grants Flying to this creature until end of turn. Do not add rules prose, art, scans, or flavor text.

The isolated evaluator compiles the module into the RAV registry and runs a full constructed PvP match. It checks the actual black cast payment, a real opposing Watchwolf cast with green and white mana, blue plus generic activation payment, stack resolution, live Flying combat, a rejected ground-creature block with no event-log mutation, cleanup expiry, reactivation, deterministic replay, and terminal game state. Replacing Flying with Reach must fail at the block challenge.
