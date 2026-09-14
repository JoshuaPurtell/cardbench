# Pokemon engine lanes

`engine_pins.toml` is the single source of truth for Pokemon engine selection.
Every benchmark package must name one `pokemon_engine_lane.id`; it must not
infer a revision from the current engine branch or silently use the newest
entry.

The `pokemon-ex-v1-frozen` lane is the vendored engine at
`varieties/pokemon/engine`. Existing Crystal Guardians and
draft-build-play-v2 evidence remains attached to that exact revision. Do not
repin or overwrite it when adding an expansion.

`pokemon-ex-power-keepers-v1` is a separate, source-pinned successor lane. It
adds generic shared-engine primitives needed by newer expansion work. CardBench
publishes only the source revision, public catalog/specification material, and
starting stubs. Expansion implementations and evaluator authority stay outside
the public candidate workspace.

Before packaging a lane, validate the source checkout and its component Git
trees:

```bash
python3 scripts/validate_engine_lanes.py
python3 scripts/validate_engine_lanes.py \
  --lane pokemon-ex-power-keepers-v1 \
  --source-checkout /path/to/setbench-engine-core
```

The first command validates the manifest and the frozen vendored lane. The
second additionally requires the supplied checkout to be exactly at the lane's
commit and verifies its component tree objects. This makes a wrong checkout or
mutable branch fail closed.
