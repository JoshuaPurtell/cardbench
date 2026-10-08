Implement Tattered Drake in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/tattered_drake.rs` by providing `definition()` and `activated_ability()`.

Tattered Drake is a blue 2/2 creature with Flying that costs `{4}{U}` and has `{B}: Regenerate Tattered Drake`. The full-game reference trace checks casting and live characteristics, self-regeneration in response to Char, shield use and survival, then a legal attack where a ground Elves of Deep Shadow cannot block. It replays the match and detects wrong-color and Flying-removal mutants.

Change only the candidate module. The diagnostic task reports 1/291 names, zero credits, and zero reward.
