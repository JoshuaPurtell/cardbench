Implement Barbarian Riftcutter in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/barbarian_riftcutter.rs` by providing `definition()` and `activated_ability()`.

The card is a red 3/3 creature that costs `{4}{R}`. Its activated ability costs `{R}`, sacrifices Barbarian Riftcutter as a cost, targets a land, and destroys that land. The deterministic reference game casts and attacks with Riftcutter, then activates it against a legal opposing land and reaches a terminal seat-zero win.

Change only the candidate module. The reference validates ordered typed payment, cast, resolution, entry, attack, combat damage, activation, sacrifice, destruction, and terminal receipts; deterministic replay; and three compiled semantic mutants. The report is diagnostic-only at 1/291, with zero card credit and zero reward.
