#!/bin/sh
set -eu

python3 varieties/magic/scripts/score_barbarian_riftcutter_pvp.py \
  --workspace "$CARDBENCH_WORKSPACE" \
  --manifest "$CARDBENCH_WORKSPACE_MANIFEST" \
  --output-root "$CARDBENCH_OUTPUT_ROOT" \
  --candidate-kind agent
