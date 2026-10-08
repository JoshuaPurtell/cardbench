#!/usr/bin/env bash
set -euo pipefail

: "${CARDBENCH_WORKSPACE_ROOT:?CARDBENCH_WORKSPACE_ROOT is required}"
CANDIDATE="${CARDBENCH_WORKSPACE_ROOT}/varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs"
if [[ ! -f "${CANDIDATE}" ]]; then
  echo "missing Char source: ${CANDIDATE}" >&2
  exit 1
fi

# The trusted Harbor adapter runs the actual compile-and-trace scorer after
# the agent returns. This task-side smoke only verifies the submission path;
# it never runs untrusted Rust in the task container.
mkdir -p /logs/verifier
printf '0.0\n' > /logs/verifier/reward.txt
echo "Char source present; isolated Harbor scoring follows submission."
