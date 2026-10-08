# Implement Peel from Reality

Implement `varieties/magic/sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs` as a literal-only module exposing `definition()`. Preserve the RAV #61 identity, blue instant type and `{1}{U}` cost, then resolve the controlled-creature return before the opponent-creature return.

The full-game authority targets a Bird of Paradise controlled by the caster and a War-Torch Goblin controlled by the opponent. It checks target order, payment, the move of each target into its owner's hand, a seat-zero terminal win, and deterministic replay. Target-scope, missing-instruction, and cost mutations must be detected.
