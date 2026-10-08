#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/seismic_spike.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Seismic Spike source: ${candidate}" >&2
  exit 1
fi
echo "Seismic Spike module present; isolated Harbor scoring follows submission."
