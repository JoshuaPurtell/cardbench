#!/usr/bin/env bash
# Harbor verifier for cardbench/pokemon/code_policy.
#
# Scores the agent's candidate on the SEALED HELDOUT split. That split is
# deliberately absent from the agent workspace, so this script must run the
# evaluator out of the host cardbench checkout, not out of the workspace copy.
#
# Both paths arrive by environment because the shared evals Harbor runner stages
# this directory into the workspace and rewrites literal "/tests/", "/logs/" and
# "/workdir/" substrings inside it. Deriving paths from $BASH_SOURCE or writing
# a literal log path would break under that rewrite.
set -euo pipefail

: "${CARDBENCH_REPO_ROOT:?CARDBENCH_REPO_ROOT must point at the host cardbench checkout}"
: "${CARDBENCH_WORKSPACE_ROOT:?CARDBENCH_WORKSPACE_ROOT must point at the agent workspace}"
: "${CARDBENCH_VERIFIER_OUT:?CARDBENCH_VERIFIER_OUT must point at the verifier output directory}"

CANDIDATE="${CARDBENCH_WORKSPACE_ROOT}/candidate/policy.rs"
SWEEP="${CARDBENCH_REPO_ROOT}/varieties/pokemon/scripts/run_policy_sweep.py"

mkdir -p "${CARDBENCH_VERIFIER_OUT}"

if [[ ! -f "${CANDIDATE}" ]]; then
  # Emit authority anyway: a missing submission is a scored zero, not a crash.
  printf '%s\n' '{"schema_version":"cardbench.harbor.result.v1","task_id":"cardbench/pokemon/code_policy","passed":false,"harbor_reward":0.0,"error":"missing candidate/policy.rs"}' \
    > "${CARDBENCH_VERIFIER_OUT}/result.json"
  printf '0.0\n' > "${CARDBENCH_VERIFIER_OUT}/reward.txt"
  exit 1
fi

exec python3 "${SWEEP}" \
  --candidate "${CANDIDATE}" \
  --candidate-id agent \
  --split heldout \
  --output-root "${CARDBENCH_VERIFIER_OUT}"
