#!/usr/bin/env bash
# Train-split feedback loop, from wherever the agent happens to be running.
#
# The graded sweep compiles Rust. When this workspace is extracted onto the
# Harbor code-policy platform the agent iterates in the PLATFORM container,
# which is `python:3.12-slim` + node + docker-cli and has no Rust toolchain at
# all -- so `run_policy_sweep.py` cannot be invoked there directly. This script
# takes whichever path is actually available:
#
#   1. cargo on PATH  -> run the sweep in place (a checkout, or a shell inside
#                        the trial image itself).
#   2. no cargo       -> re-run it inside the trial image as a sibling
#                        container on the host daemon, with this workspace
#                        bind-mounted. Needs the docker socket, the trial image
#                        reference (CARDBENCH_TRIAL_IMAGE) and the
#                        platform->host workspace mapping the launcher exports
#                        as SYNTH_WORKSPACE_ROOT / SYNTH_WORKSPACE_HOST_ROOT.
#
# Either way this scores the TRAIN split only. The heldout split is not in this
# workspace and no argument here can reach it.
set -euo pipefail

WORKSPACE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CANDIDATE="${1:-${WORKSPACE}/candidate/policy.rs}"
OUTPUT="${2:-${WORKSPACE}/artifacts/train}"

if [[ ! -f "${CANDIDATE}" ]]; then
  echo "no candidate at ${CANDIDATE}; write candidate/policy.rs first" >&2
  exit 2
fi

if command -v cargo >/dev/null 2>&1; then
  exec python3 "${WORKSPACE}/varieties/pokemon/scripts/run_policy_sweep.py" \
    --candidate "${CANDIDATE}" --split train --output-root "${OUTPUT}"
fi

IMAGE="${CARDBENCH_TRIAL_IMAGE:-}"
MOUNT_ROOT="${SYNTH_WORKSPACE_ROOT:-}"
HOST_ROOT="${SYNTH_WORKSPACE_HOST_ROOT:-}"
for name in IMAGE MOUNT_ROOT HOST_ROOT; do
  if [[ -z "${!name}" ]]; then
    echo "no cargo here and no container fallback: ${name} is unset" >&2
    exit 3
  fi
done
case "${WORKSPACE}" in
  "${MOUNT_ROOT}"/*) RELATIVE="${WORKSPACE#"${MOUNT_ROOT}"/}" ;;
  *) echo "workspace ${WORKSPACE} is outside ${MOUNT_ROOT}; cannot map to a host path" >&2
     exit 3 ;;
esac
HOST_WORKSPACE="${HOST_ROOT}/${RELATIVE}"

# --network none: the sweep needs nothing from the network, and the candidate
# it compiles is untrusted.
exec docker run --rm --network none \
  -v "${HOST_WORKSPACE}:/workspace" \
  -e CARDBENCH_CARGO_CACHE=/opt/cardbench/cargo \
  "${IMAGE}" \
  python3 /task/cardbench/varieties/pokemon/scripts/run_policy_sweep.py \
  --candidate "/workspace/${CANDIDATE#"${WORKSPACE}"/}" \
  --split train \
  --output-root "/workspace/${OUTPUT#"${WORKSPACE}"/}"
