#!/usr/bin/env bash
set -euo pipefail

: "${CARDBENCH_WORKSPACE_ROOT:?CARDBENCH_WORKSPACE_ROOT is required}"
CANDIDATE="${CARDBENCH_WORKSPACE_ROOT}/varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs"
if [[ ! -f "${CANDIDATE}" ]]; then
  echo "missing Lightning Helix source: ${CANDIDATE}" >&2
  exit 1
fi

# The trusted Harbor adapter runs the compile-and-trace scorer after the agent
# returns. This task-side smoke only checks the submission path and never runs
# untrusted Rust in the task container.
mkdir -p /logs/verifier
printf '0.0\n' > /logs/verifier/reward.txt
echo "Lightning Helix source present; isolated Harbor scoring follows submission."
