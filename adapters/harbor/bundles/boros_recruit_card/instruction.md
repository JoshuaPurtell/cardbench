# CardBench Magic single-card task: Boros Recruit

Implement the RAV Boros Recruit definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_recruit.rs`.

Printed characteristics: `{R/W}`, red and white, Creature, 1/1, First Strike. Preserve the literal-only declarative `definition() -> CardDefinition` source boundary and existing field layout.

The verifier plays two legal shuffled 60-card games: one pays the hybrid symbol with red mana, the other with white mana. Each trace records casting, resolution, and battlefield entry. In later PvP combat Recruit attacks and Elves of Deep Shadow legally blocks it. The ordered event receipts must show First Strike damage dealing one damage to the 1/1 blocker, the blocker dying before normal combat damage, and Recruit remaining tapped but undamaged. The report is diagnostic-only against the 291-name/306-printing RAV catalog and awards no set coverage or Harbor reward.
