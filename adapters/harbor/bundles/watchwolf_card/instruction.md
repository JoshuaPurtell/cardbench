# CardBench Magic single-card task: Watchwolf

Implement the RAV Watchwolf definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/watchwolf.rs`.

The card is RAV collector #239: `{G}{W}`, green-white Creature, 3/3, with no
keywords or functional abilities. Preserve the pure declarative
`definition() -> CardDefinition` source boundary and the existing field layout.

The verifier plays a legal shuffled 60-card constructed game, records the
Watchwolf cast, resolution, battlefield entry, and terminal match event, checks
that both green and white mana were available for the accepted cast and checks
the live 3/3 characteristics, then replays the same seed. The shown Selesnya
Midrange deck faces the shown Boros Aggro deck
with a deterministic pass-only opponent policy. The result is a one-card
diagnostic against 291 unique RAV names and 306 printings; it does not credit
set coverage or Harbor reward.
