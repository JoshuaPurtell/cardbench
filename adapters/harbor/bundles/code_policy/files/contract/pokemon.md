## Candidate contract

A candidate is exactly ONE file: `candidate/policy.rs`, defining one public
struct with `new(seed: u64) -> Self` and an `impl tcg_ai::AiController`.

It is compiled into the benchmark binary, so it may use `tcg_core`, `tcg_ai`,
`rand` and `rand_chacha`. Nothing else in this workspace is on its path.

The ranking origin is
`varieties/pokemon/candidates/reference/reference_policy_v2.rs`. It is a working
example and the thing you are measured against — start from it.

The held-out split is 4 decks and 8 opponent policies you cannot see: 384 cells.
