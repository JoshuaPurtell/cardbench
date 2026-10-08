#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_fire_fiend.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Goblin Fire Fiend source: ${candidate}" >&2
  exit 1
fi
echo "Goblin Fire Fiend module present; isolated Harbor scoring follows submission."
