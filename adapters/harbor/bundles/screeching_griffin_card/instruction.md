Implement the RAV Screeching Griffin candidate module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/screeching_griffin.rs`.

Keep the module declarative. It owns the printed white 2/2 creature definition with mana cost `{3}{W}` and Flying, plus the targeted red-mana activated ability that prevents its target from blocking Screeching Griffin for the turn. Do not add rules prose, art, scans, or flavor text.

The isolated evaluator compiles the candidate module into the RAV registry and runs a legal constructed PvP match. It checks both creatures' actual colored and generic cast payments, the activated ability's red payment and stack resolution, the live Skyknight target, source-relative effect, rejected blocking attempt with an unchanged event log, combat receipts, terminal result, and deterministic replay. Replacing the blocking restriction with a harmless target +1/+1 effect must fail at the block challenge.
