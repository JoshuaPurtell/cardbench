#!/bin/sh
set -eu

python3 varieties/magic/scripts/score_golgari_rotwurm_pvp.py \
  --workspace "$CARDBENCH_WORKSPACE" \
  --manifest "$CARDBENCH_WORKSPACE_MANIFEST" \
  --output-root "$CARDBENCH_OUTPUT_ROOT" \
  --candidate-kind agent
