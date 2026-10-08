# CardBench Magic single-card task: Selesnya Sagittars

Implement Selesnya Sagittars in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_sagittars.rs` by providing `definition()` and `tap_damage_ability()`.

The card is a green-white 2/5 creature costing `{3}{G}{W}` with Reach. Its tap ability deals one damage to target attacking or blocking creature.

The authority runs a legal shuffled full-game trace. It checks exact mana paid, cast/resolution/entry, a Reach block against flying Courier Hawk, an activation targeting attacking Elves of Deep Shadow, target damage and removal before combat damage, the Hawk combat exchange, deterministic replay, and a later Watchwolf game win. A separate legality probe confirms a noncombat creature is not a legal target and that rejection is atomic. Semantic mutants remove damage, the tap cost, Reach, or the target boundary.

The task is diagnostic-only: one trace-passed name from 291 unique RAV names / 306 printings, zero credited cards, and zero reward. Do not change files outside the candidate module.
