#!/usr/bin/env bash
set -euo pipefail
candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/drift_of_phantasms.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Drift of Phantasms source: ${candidate}" >&2
  exit 1
fi
echo "Drift of Phantasms module present; isolated Harbor scoring follows submission."
