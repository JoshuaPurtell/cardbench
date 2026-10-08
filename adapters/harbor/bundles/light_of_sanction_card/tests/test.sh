#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-}"
if [[ -z "$candidate" ]]; then
  candidate="varieties/magic/sets/ravnica_city_of_guilds/src/cards/light_of_sanction.rs"
fi
if [[ ! -f "$candidate" ]]; then
  echo "missing Light of Sanction source: $candidate" >&2
  exit 1
fi

echo "Light of Sanction source present; isolated Harbor scoring follows submission."
