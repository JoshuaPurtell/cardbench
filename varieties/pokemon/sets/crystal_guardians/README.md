# Crystal Guardians

This directory is the source-of-truth benchmark family for Crystal Guardians.

## Layout

- `source/`: canonical benchmark assets.
- `variants/`: partial-gold curriculum tiers derived from the canonical source.

This family also respects the repo-level split:

- `setbench-engine-core` supplies pinned shared engine code
- `setbench/crystal_guardians/source` owns the benchmark source of truth
- `setbench/crystal_guardians/variants` owns derived disclosure tiers
- `terminal-bench-3` should only receive one exported packaged variant

## Canonical source assets

The canonical source contains:

- public train data and the visible V1 runner
- hidden held-out scenarios and the hidden V4 runner
- the two gold files
- the two agent-implemented stub files
- task metadata used by exported tasks
- `family.toml`, which defines the graded files, runner paths, and disclosure rules

## Variant semantics

The `0pct`, `30pct`, and `80pct` variants are approximate disclosure tiers based on public function exposure across the two graded files.

- `0pct`: current Terminal Bench task shape, with both graded files fully stubbed.
- `30pct`: static wiring and a small portion of engine glue are revealed.
- `80pct`: most public hook surface is revealed, while the hardest runtime behaviors remain stubbed.

The percentages are intentionally based on the public graded hook surface, not raw line counts. The current Crystal Guardians graded surface has 25 public functions across the two files.

## Relationship to Terminal Bench

`terminal-bench-3` should only receive one exported variant at a time.

For the current public task, that export is Crystal Guardians `0pct`. The TB3 task must stay decoupled from this repo and continue pinning `setbench-engine-core` directly.
