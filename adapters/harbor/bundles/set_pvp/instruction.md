# CardBench Magic set-PvP pilot: RAV Char

Implement the CardBench-authored Char feature summary in
`varieties/magic/tasks/rav-char-pvp/features.toml`. Edit only:

`varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs`

The task workspace starts with a compile-valid stub. Keep the implementation
inside the existing `definition() -> CardDefinition` boundary. Do not change
the engine, policies, task runner, reference verifier, catalog, or other card
modules.

The shown full-game traces cover a player target and an opponent creature
target. The verifier checks a legal cast, target legality, the four-damage
instruction, two damage to the controller, event order, the post-resolution
life checkpoint, the graveyard move, terminal completion, deterministic
replay, and rejection of a 4-to-3 semantic mutation.

The candidate is compiled and executed in Harbor's isolated offline Rust
worker. Do not run candidate code or its compiler on the host. The result is
reported against all 291 RAV names / 306 printings; only the Char pilot can
pass, and the whole-set reward remains zero until the full denominator is
qualified.

The source contract follows the RAV printing's original rules function;
current Oracle wording is a cross-check, and later target-domain errata are
excluded from this original-set compatibility pilot. The rules baseline and cutoff are recorded in
`varieties/magic/tasks/rav-char-pvp/README.md` and `features.toml`. This is a
one-card pilot, not a claim of full RAV, GPT, or DIS readiness.
