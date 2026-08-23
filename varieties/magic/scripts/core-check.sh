#!/bin/sh
# Bounded core validation gate for the edit loop.
#
# The exhaustive package/workspace suites remain release checks.  This gate
# keeps the high-signal engine, policy-boundary, catalog, and adversarial audit
# checks on a reusable target directory and fails if the local run regresses
# past the agreed 30-second budget.
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CORE_CHECK_BUDGET_SECONDS=${CORE_CHECK_BUDGET_SECONDS:-30}
start_seconds=$(date +%s)

cd "$ROOT"
./scripts/fast-check.sh

end_seconds=$(date +%s)
elapsed_seconds=$((end_seconds - start_seconds))
printf 'core_check_elapsed_seconds=%s budget_seconds=%s\n' \
  "$elapsed_seconds" "$CORE_CHECK_BUDGET_SECONDS"
if [ "$elapsed_seconds" -gt "$CORE_CHECK_BUDGET_SECONDS" ]; then
  printf >&2 'core check exceeded its %s-second budget\n' \
    "$CORE_CHECK_BUDGET_SECONDS"
  exit 1
fi
