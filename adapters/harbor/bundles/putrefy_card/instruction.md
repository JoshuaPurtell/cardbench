# Implement Putrefy

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/putrefy.rs` as a literal-only module exposing `definition()`. Preserve the RAV #221 identity, `{1}{B}{G}` black-green instant characteristics, and the artifact-or-creature destruction instruction that bypasses regeneration.

The shuffled PvP reference casts Putrefy at a Glass Golem after Votary of the Conclave creates a regeneration shield for it. The trace must show actual generic, black, and green payment; destruction of the targeted permanent; no regeneration-shield use; deterministic replay; and a terminal pilot win. Mutations removing destruction, allowing regeneration, or changing the printed cost must fail.
