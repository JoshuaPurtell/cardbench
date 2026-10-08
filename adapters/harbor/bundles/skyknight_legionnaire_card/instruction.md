# CardBench Magic single-card task: Skyknight Legionnaire

Implement the RAV Skyknight Legionnaire definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/skyknight_legionnaire.rs`.

The card is RAV collector #232: `{1}{R}{W}`, red-white Creature, 2/2, with
Flying and Haste. Keep the source as one pure declarative
`definition() -> CardDefinition` literal with the existing fields.

The trusted verifier plays a seeded legal 60-card match, casts and resolves the
creature, checks its red, white, and generic mana receipts and live
characteristics, then requires the same object to attack in the turn it entered.
It records ordered spell and combat events and replays the seed. Removing Haste
must fail because a summoning-sick creature cannot attack immediately without
Haste. The report is a one-card diagnostic over all 291 unique RAV names and
306 printings; it does not award whole-set coverage or Harbor reward.
