Implement Sewerdreg in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/sewerdreg.rs` by providing `definition()` and `activated_ability()`.

Sewerdreg is a black 3/3 creature costing `{3}{B}{B}` with Fear and `{B}: Regenerate Sewerdreg`. The full-game authority checks the exact cast cost and live Fear, self-regeneration in response to lethal Char damage, and a failed block probe by a nonblack nonartifact Elves of Deep Shadow. It checks the rejected probe leaves the game unchanged, then completes combat and a seat-zero win. It also verifies deterministic replay and compiled wrong-cost and Fear-removal mutants.

Change only the candidate module. The diagnostic task reports 1/291 names, zero credits, and zero reward.
