#!/usr/bin/env bash
# Harbor verifier for cardbench/<variety>/code_policy.
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
# Baked by the Dockerfile from its build arg. Required rather than defaulted:
# this script hardcoded `pokemon` in both the sweep path and the task id, and a
# default would let a second variety bake successfully and then silently grade
# against the first one's roster.
: "${CARDBENCH_VARIETY:?CARDBENCH_VARIETY must name the variety this image was baked for}"

CANDIDATE="${CARDBENCH_WORKSPACE_ROOT}/candidate/policy.rs"
SWEEP="${CARDBENCH_REPO_ROOT}/varieties/${CARDBENCH_VARIETY}/scripts/run_policy_sweep.py"

mkdir -p "${CARDBENCH_VERIFIER_OUT}"

if [[ ! -f "${SWEEP}" ]]; then
  # A variety with no driver is a bake that should never have happened, and it
  # is ours, not the candidate's: emit no reward at all rather than a zero.
  printf '{"schema_version":"cardbench.harbor.result.v1","task_id":"cardbench/%s/code_policy","passed":false,"reward_status":"harness_failed","error":"no run_policy_sweep.py for variety %s"}\n' \
    "${CARDBENCH_VARIETY}" "${CARDBENCH_VARIETY}" > "${CARDBENCH_VERIFIER_OUT}/result.json"
  exit 2
fi

if [[ ! -f "${CANDIDATE}" ]]; then
  # Emit authority anyway: a missing submission is a scored zero, not a crash.
  printf '{"schema_version":"cardbench.harbor.result.v1","task_id":"cardbench/%s/code_policy","passed":false,"harbor_reward":0.0,"reward_status":"scored","error":"missing candidate/policy.rs"}\n' \
    "${CARDBENCH_VARIETY}" > "${CARDBENCH_VERIFIER_OUT}/result.json"
  printf '0.0\n' > "${CARDBENCH_VERIFIER_OUT}/reward.txt"
  exit 1
fi

exec python3 "${SWEEP}" \
  --candidate "${CANDIDATE}" \
  --candidate-id agent \
  --split heldout \
  --output-root "${CARDBENCH_VERIFIER_OUT}"
