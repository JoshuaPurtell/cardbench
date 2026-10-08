# CardBench Magic single-card task: Lightning Helix

Implement the CardBench-authored feature summary in
`varieties/magic/tasks/rav-lightning-helix-pvp/features.toml`. Edit only:

`varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs`

The task workspace starts with a compile-valid stub. Keep the implementation
inside the existing pure, declarative `definition() -> CardDefinition`
boundary, using the file's existing imports and field layout. Do not add helper
logic or side effects. The verifier rejects extra items and executable code.
Do not change the engine, policies, task runner, reference verifier, catalog,
or other card modules.

The shown full-game traces cover a player target and an opponent creature
target. The verifier checks a legal cast, three damage to the selected target,
three life gained by the controller, event order, post-resolution life state,
graveyard movement, terminal completion, deterministic replay, and rejection
of a 3-to-2 semantic mutation.

The candidate is compiled and executed in Harbor's isolated offline Rust
worker. Do not run candidate code or its compiler on the host. The report uses
the complete RAV denominator; one Lightning Helix pass does not imply whole-set
readiness. The source contract follows the RAV printing's original target
domain and records its rules baseline in the task README and feature contract.
