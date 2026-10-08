# CardBench Magic single-card task: Goliath Spider

Implement the RAV Goliath Spider definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/goliath_spider.rs`.
The card is collector #168, a green 7/6 creature with cost `{6}{G}{G}` and
Reach.

The trusted verifier runs a seeded legal 60-card PvP match. It casts and
resolves Goliath Spider, then has a real opposing Skyknight Legionnaire attack
while the Spider can block. The block must be accepted because Goliath Spider
has Reach. The combat damage must kill the 2/2 flyer while the Spider remains
live; the game then continues to a terminal result. Ordered game events,
actual cast receipts, deterministic replay, a Reach-removal mutation, and a
compile-valid stub rejection are checked.

Keep the source as a pure declarative definition. Do not include copied card
rules prose, artwork, or flavor text. This task diagnoses one card among all
291 RAV identities and 306 printings; it awards no whole-set coverage or Harbor
reward.
