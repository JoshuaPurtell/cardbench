# CardBench Magic single-card task: Centaur Safeguard

Implement the RAV Centaur Safeguard definition in
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/centaur_safeguard.rs`.

The card is collector #244: a green/white 3/1 creature costing two green/white hybrid symbols. Keep both functions declarative: `definition() -> CardDefinition` and the candidate-owned `triggered_ability() -> TriggeredAbilityBinding`.

Implement both `definition() -> CardDefinition` and the candidate-owned
`triggered_ability() -> TriggeredAbilityBinding` in that module. The binding
must describe the optional dies trigger that gains three life.

The verifier casts the card from a legal shuffled 60-card Selesnya deck. Elves
of Deep Shadow from a Golgari deck blocks it. Typed events must show hybrid
payment, combat damage, Safeguard's graveyard move, its source-incarnation dies
trigger being stacked, and both legal choices: accepting gains exactly three
life, while declining gains no life from the trigger. Both traces replay
deterministically. The task is diagnostic-only against 291 unique RAV names
and 306 printings; it credits no set coverage or Harbor reward.
