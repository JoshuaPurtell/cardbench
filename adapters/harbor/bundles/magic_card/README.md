# Magic single-card Harbor bundle

This bundle currently carries the RAV Lightning Helix pilot. The task workspace
is materialized from its source contract with one compile-valid stub at
`varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs`.
Harbor verifies the candidate workspace against its manifest and runs the
candidate in an offline, networkless Rust worker with no host credential or
heldout-data mount.

The Codex route requires the shared runner's Linux UID isolation and verifies
that the agent identity cannot read the host checkout or Git metadata. It stops
before launch if that boundary cannot be proven. The task smoke writes zero
reward; only the trusted post-submission scorer can pass the single-card pilot.
The report retains the full 291-name / 306-printing RAV denominator and does not
claim whole-set coverage.
