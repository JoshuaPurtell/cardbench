# CardBench Magic single-card task: Temple Garden

Implement RAV #284 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/temple_garden.rs`.
Keep the module to the candidate-owned card definition, green/white mana
binding, and optional entry-life binding.

The trusted verifier runs a shuffled 60-card two-player duel. It requires one
Temple Garden to enter untapped after its controller pays two life and a
second to enter tapped after that payment is declined. The untapped land must
produce both green and white mana used for real creature spells. The complete
game must reach a terminal result and replay exactly. This task does not award
card credit or Harbor reward.
