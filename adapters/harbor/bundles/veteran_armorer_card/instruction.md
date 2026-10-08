Implement the candidate-owned RAV Veteran Armorer module at `varieties/magic/sets/ravnica_city_of_guilds/src/cards/veteran_armorer.rs`.

Keep the module declarative. It owns the white 2/2 creature definition with printed cost `{1}{W}` and its static +0/+1 modification to other creatures its controller controls. Do not add rules prose, art, scans, or flavor text.

The isolated evaluator runs a full constructed PvP game. It casts an actual green/white Watchwolf first and records its 3/3 state, then casts Veteran Armorer and verifies the same controlled Watchwolf becomes 3/4 while Veteran Armorer itself remains 2/2. The trace checks ordered cast and combat events, terminal game state, deterministic replay, and the targeted +0/+0 mutation.
