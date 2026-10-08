#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-}"
if [[ -z "$candidate" ]]; then
  candidate="varieties/magic/sets/ravnica_city_of_guilds/src/cards/privileged_position.rs"
fi
if [[ ! -f "$candidate" ]]; then
  echo "missing Privileged Position source: $candidate" >&2
  exit 1
fi

echo "Privileged Position source present; isolated Harbor scoring follows submission."
