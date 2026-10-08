# CardBench Magic single-card task: Courier Hawk

Implement the RAV Courier Hawk definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/courier_hawk.rs`.

Courier Hawk is RAV collector #12: `{1}{W}`, white Creature, 1/2, with Flying
and Vigilance. Keep the implementation as a pure declarative
`definition() -> CardDefinition` literal with the existing fields.

The verifier plays a shuffled 60-card Selesnya curve deck against a 60-card
Boros Aggro deck using a deterministic pass-only opponent. It records the Hawk
cast, resolution, battlefield entry, and terminal match event; checks the paid
white and generic mana; and reads the permanent's live power, toughness, Flying,
and Vigilance after resolution. The same seed is replayed. This is a one-card
diagnostic against all 291 unique RAV names and 306 printings; it does not award
set coverage or Harbor reward.
