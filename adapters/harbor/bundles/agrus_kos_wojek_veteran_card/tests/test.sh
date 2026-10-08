#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/agrus_kos_wojek_veteran.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Agrus Kos candidate module: ${candidate}" >&2
  exit 1
fi
echo "Agrus Kos candidate module is present; the trusted authority checks attack-triggered color modifiers and the full-game trace."
