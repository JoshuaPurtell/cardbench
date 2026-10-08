# CardBench Magic single-card task: Boros Swiftblade

Implement the RAV Boros Swiftblade definition in `varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_swiftblade.rs`.

Printed characteristics: `{R}{W}`, red and white, Creature, 1/2, Double Strike. Preserve the literal-only declarative `definition() -> CardDefinition` source boundary and existing field layout.

The verifier plays a legal shuffled 60-card constructed game, records one red and one white mana paid, then traces casting, resolution, battlefield entry, and an actual attack. The opposing player legally declares no blockers. Ordered typed events must show Swiftblade dealing one damage to that player during First Strike combat damage and one more during normal combat damage. The report is diagnostic-only against the 291-name/306-printing RAV catalog and awards no set coverage or Harbor reward.
