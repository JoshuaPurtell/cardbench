# CardBench agent notes

Read `/Users/joshuapurtell/Documents/GitHub/evals/tcg.md` before changing the
benchmark.

Hard boundaries:

- CardBench task ids are `cardbench/<variety>/<family>`.
- Never put CardBench content under `gamebench/tasks/`.
- Never register these tasks with `bench = "gamebench"`.
- Keep `code_policy`, `deck_opt`, `engine`, `react`, and `cybernetic` rewards
  separate.
- Deck evaluation always uses a pinned code policy.
- ReAct opponents must be published code-policy ids.
- Pixel visualization is evidence, not grading authority.
- Expansion-specific rules live under the owning variety and expansion; do
  not force Magic stack/priority semantics into the Pokémon engine.

Do not mark a lane ready until its reference verify is green and the expected
authority artifact exists.
