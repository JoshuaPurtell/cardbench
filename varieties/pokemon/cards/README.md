# Pokémon single-card authoring

`cardbench/pokemon/card` restores the EngineBench microtask: implement one
card module without changing the shared engine.

- `instances/` contains the public card specification and named behaviors.
- `stubs/` contains the candidate starting point.
- `catalog.json` pins hashes for the sealed reference implementation and
  deterministic verifier tests.
- `../scripts/run_card_eval.py` materializes an isolated engine workspace,
  injects the selected tests, compiles `tcg_expansions`, and scores test pass
  rate behind a compile gate.

The stable selector is the instance id, for example
`df-097-rayquaza-ex`. A future expansion adds instances and card modules; it
does not add a new task family.

Gold and evaluator Rust are deliberately absent from this public tree. Set
`CARDBENCH_SEALED_ROOT` to a trusted directory containing
`pokemon/card/{implementations,tests}` for reference verification.
