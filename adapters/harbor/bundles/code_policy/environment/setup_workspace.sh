#!/usr/bin/env bash
# Build the agent's /workspace copy out of the graded /task copy.
#
# The two copies differ in exactly two ways, and both are deliberate:
#   * `/workspace` has no `.sealed/` -- the 4 heldout decks, the 8 heldout
#     opponent policies, the heldout server db and the cell manifest. Held out
#     has to mean held out.
#   * `/workspace` has the empty `candidate/` directory the verifier reads.
# Everything else is the same checkout, so what the agent iterates against is
# what it is graded with.
set -euo pipefail

TASK_REPO="${CARDBENCH_REPO_ROOT:-/task/cardbench}"
WORKSPACE="${CARDBENCH_WORKSPACE_ROOT:-/workspace}"

mkdir -p "${WORKSPACE}"
cp -a "${TASK_REPO}/." "${WORKSPACE}/"

# Mirrors run_harbor.py's EXCLUDED_WORKSPACE_DIRS: the non-container lane has
# always staged the repo minus these, and the container lane must not hand the
# agent more than the lane it replaces.
find "${WORKSPACE}" -depth -type d \
  \( -name .sealed -o -name .git -o -name artifacts -o -name target \
     -o -name .cache -o -name __pycache__ \) \
  -exec rm -rf {} +

mkdir -p "${WORKSPACE}/candidate" "${WORKSPACE}/logs/verifier"

# Fail the bake, not the run. run_harbor.py's assert_no_sealed_leak does this
# for the non-container lane; a container lane that skipped it would ship a
# workspace whose "heldout" claim nobody ever checked.
LEAKED="$(find "${WORKSPACE}" -name .sealed -print -quit)"
if [[ -n "${LEAKED}" ]]; then
  echo "sealed heldout assets leaked into the agent workspace: ${LEAKED}" >&2
  exit 1
fi
if [[ ! -d "${TASK_REPO}/varieties/pokemon/.sealed/code_policy" ]]; then
  echo "graded copy has no sealed heldout split: ${TASK_REPO}" >&2
  exit 1
fi
