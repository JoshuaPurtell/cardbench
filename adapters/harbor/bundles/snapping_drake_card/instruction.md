# CardBench Magic single-card task: Snapping Drake

Implement the RAV Snapping Drake definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/snapping_drake.rs`.

The card is RAV collector #64: `{3}{U}`, blue Creature, 3/2, with Flying and
no functional abilities. Preserve the pure declarative
`definition() -> CardDefinition` source boundary and the existing field layout.

The verifier plays a legal shuffled 60-card constructed match using a task-local
deck containing four Snapping Drakes and 56 Islands against the public Boros
Aggro 60-card deck. It records the Snapping Drake cast, resolution, battlefield
entry, terminal match event, exact blue mana payment, and live 3/2 Flying
characteristics. It also records a legal attack after the entry turn, then
replays the same seed. Cost, power, toughness, and Flying mutants are checked
through the full-game trace. The result is a one-card
diagnostic against 291 unique RAV names and 306 printings; it does not credit
set coverage or Harbor reward.
