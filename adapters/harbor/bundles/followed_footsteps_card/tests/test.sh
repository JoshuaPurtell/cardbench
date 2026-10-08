#!/bin/sh
set -eu

python3 varieties/magic/scripts/materialize_followed_footsteps_task.py \
  --workspace /tmp/cardbench-followed-footsteps-workspace \
  --manifest /tmp/cardbench-followed-footsteps-manifest.json
python3 varieties/magic/scripts/score_followed_footsteps_pvp.py \
  --workspace /tmp/cardbench-followed-footsteps-workspace \
  --manifest /tmp/cardbench-followed-footsteps-manifest.json \
  --output-root /tmp/cardbench-followed-footsteps-output
