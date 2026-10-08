# CardBench Magic single-card task: Overgrown Tomb

Implement RAV #279 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/overgrown_tomb.rs`.
Keep the module to the candidate-owned card definition, black/green mana
binding, and optional entry-life binding.

The trusted verifier runs a shuffled 60-card two-player duel. One Overgrown
Tomb must enter untapped after its controller pays two life; a second must
enter tapped after that payment is declined. The paid Tomb must produce Green
for Elves of Deep Shadow ({G}) and Black for Roofstalker Wight ({1}{B}); use a
Forest for the Wight's generic pip. The complete game must reach a terminal
result and replay exactly. This task grants no card credit or Harbor reward.
