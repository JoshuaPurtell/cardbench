# CardBench Magic single-card task: Glass Golem

Implement the RAV Glass Golem definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/glass_golem.rs`.

The card is RAV collector #261: `{5}`, colorless Artifact Creature, 6/2, with
no keywords or functional abilities. Preserve the pure declarative
`definition() -> CardDefinition` source boundary and the existing field layout.

The verifier plays a legal shuffled 60-card constructed game, records the
Glass Golem cast, resolution, battlefield entry, and terminal match event, checks
that exactly five mana were available and paid for the accepted cast and checks
the live 6/2 characteristics, then replays the same seed. The shown Selesnya
Midrange deck faces the shown Boros Aggro deck
with a deterministic pass-only opponent policy. The result is a one-card
diagnostic against 291 unique RAV names and 306 printings; it does not credit
set coverage or Harbor reward.
