# CardBench Pokémon code-policy task

Write `candidate/policy.rs`: one public Rust struct with `new(seed: u64) -> Self`
and an implementation of `tcg_ai::AiController`. It must compile, produce legal
actions, and finish its games.

## How you are scored

Play is scored over a **fixed grid of cells**. One cell is a single coordinate:

```
candidate deck | opponent deck | opponent policy | seat (p1/p2) | seed
```

Your policy and the reference baseline are run over *exactly the same cells* and
compared pairwise, so a win only counts if it beats the baseline on the same
matchup — not on an easier one. The lift must clear a paired-bootstrap
confidence interval, so noise on a handful of lucky cells will not pass.

There are two splits:

| Split | Decks | Opponents | Role |
|-------|-------|-----------|------|
| **train** | the 5 decks in `varieties/pokemon/policies/data/server.sqlite` | the 5 in `rosters/code_policy_v1.json` | feedback — iterate against this |
| **heldout** | 4 decks you cannot see | 8 policies you cannot see | **authority** — this is your score |

`varieties/pokemon/rosters/code_policy_v1.json` is the contract. It names the
whole train surface and publishes the *shape* of the heldout split (4 decks, 8
opponents, 384 cells) and nothing else.

The heldout decks are built from a card pool the train decks never touch, so
tuning to specific train decklists will not transfer. Assume the heldout
opponents are stronger than the train ones.

## Iterating

```bash
# feedback signal (400 cells)
./run_train_sweep.sh
```

Use the wrapper, not `run_policy_sweep.py` directly. The sweep compiles Rust,
and depending on where this workspace is mounted you may be running somewhere
that has no Rust toolchain; the wrapper detects that and re-runs the same sweep
where a toolchain exists. Invoking the sweep directly works in some environments
and fails in others, and the failure is not obvious.

It grades the **train split only** — the heldout split is not in this workspace
and no argument reaches it.

Read `artifacts/train/per_cell.jsonl` to see which matchups you lose. The
per-opponent and per-deck breakdowns are usually more informative than the
single blended number.

## Rules

- Do not read, write, or otherwise reach into `varieties/pokemon/.sealed/`.
  It is excluded from this workspace; attempting to reconstruct it is
  disqualifying.
- Do not modify the roster, the evaluator, the reference policies, or anything
  under `varieties/pokemon/policies/data/`.
- Do not read or edit verifier output.

## Where the headroom is

`candidates/reference/reference_policy_v2.rs` is the ranking origin: it measures
64.9% on the train surface and beats all five train opponents, so a candidate has
to be genuinely better rather than merely functional. `reference_policy_v1.rs`
(35.6%) and `baseline_policy.rs` (28.9%) are kept as weaker predecessors to read,
not as the bar.

The single largest lever measured so far is **retreat discipline**: retreating
whenever the active looks endangered costs 18.3 points, because retreat pays its
cost by discarding energy — lost games ran 46 turns with 16 energy attached and
only 8.7 attacks.

The origin still refuses two things, because doing them naively *stalls games* —
and a stalled game scores as a loss:

- **Playing trainers** — measured 757/800 stalled games.
- **Evolving from hand** — measured 271/800 stalled games.

Both stall for the same reason: they raise follow-up prompts (search, discard,
reorder, choose-targets) that the reference answers with a no-op. A policy that
answers those prompts can use both, and that is the largest single piece of
value left on the table.
