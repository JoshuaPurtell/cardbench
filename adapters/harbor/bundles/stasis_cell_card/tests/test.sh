#!/bin/sh
set -eu

python3 varieties/magic/scripts/materialize_stasis_cell_task.py \
  --workspace /tmp/cardbench-stasis-cell-workspace \
  --manifest /tmp/cardbench-stasis-cell-manifest.json
python3 varieties/magic/scripts/score_stasis_cell_pvp.py \
  --workspace /tmp/cardbench-stasis-cell-workspace \
  --manifest /tmp/cardbench-stasis-cell-manifest.json \
  --output-root /tmp/cardbench-stasis-cell-output
