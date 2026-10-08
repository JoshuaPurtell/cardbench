#!/bin/sh
set -eu

python3 varieties/magic/scripts/materialize_copy_enchantment_task.py \
  --workspace /tmp/cardbench-copy-enchantment-workspace \
  --manifest /tmp/cardbench-copy-enchantment-manifest.json
python3 varieties/magic/scripts/score_copy_enchantment_pvp.py \
  --workspace /tmp/cardbench-copy-enchantment-workspace \
  --manifest /tmp/cardbench-copy-enchantment-manifest.json \
  --output-root /tmp/cardbench-copy-enchantment-output
