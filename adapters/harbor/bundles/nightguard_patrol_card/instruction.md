# CardBench Magic single-card task: Nightguard Patrol

Implement the RAV Nightguard Patrol definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/nightguard_patrol.rs`.

The card is RAV collector #26: `{2}{W}`, white Creature, 2/1, with first strike and vigilance. Preserve the pure declarative `definition() -> CardDefinition` source boundary and existing field layout.

The verifier plays a legal shuffled 60-card constructed game, records actual colored and generic mana payment, cast, resolution, battlefield entry, and terminal events, and checks the live 2/1 First Strike/Vigilance characteristics before replaying the match. On a later turn Patrol must attack without tapping, then Elves of Deep Shadow from the Golgari deck blocks it. The ordered typed trace must show two First Strike damage, the 1/1 dying before the ordinary CombatDamage step, and Patrol remaining untapped and undamaged. The result is diagnostic-only against 291 unique RAV names and 306 printings; it credits no set coverage or Harbor reward.
