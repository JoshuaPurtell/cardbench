# CardBench Pokémon code-policy task

Create `candidate/policy.rs` containing one public Rust struct with
`new(seed: u64)` and an implementation of `tcg_ai::AiController`. Read the
engine and existing policy examples in this workspace. Improve blended win
rate without reading or modifying verifier outputs. The candidate must compile,
produce legal actions, and complete scored games.
