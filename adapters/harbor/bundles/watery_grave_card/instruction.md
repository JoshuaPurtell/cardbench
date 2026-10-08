# CardBench Magic single-card task: Watery Grave

Implement RAV #286 in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/watery_grave.rs`.
Keep the module to the candidate-owned card definition, blue/black mana
binding, and optional entry-life binding.

The trusted verifier runs a shuffled 60-card two-player duel. It requires one
Watery Grave to enter untapped after its controller pays two life and a
second to enter tapped after that payment is declined. The untapped land must
produce blue from that Grave for Surveilling Sprite ({1}{U}) and black from the same Grave for Roofstalker Wight ({1}{B}), using the opposite-color basics for the generic pips. The complete
game must reach a terminal result and replay exactly. This task does not award
card credit or Harbor reward.
