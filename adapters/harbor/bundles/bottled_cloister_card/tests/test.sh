#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-}"
if [[ -z "$candidate" ]]; then
  candidate="varieties/magic/sets/ravnica_city_of_guilds/src/cards/bottled_cloister.rs"
fi
if [[ ! -f "$candidate" ]]; then
  echo "missing Bottled Cloister source: $candidate" >&2
  exit 1
fi

echo "Bottled Cloister source present; isolated Harbor scoring follows submission."
