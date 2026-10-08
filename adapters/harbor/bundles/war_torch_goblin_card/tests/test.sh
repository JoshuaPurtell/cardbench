#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/war_torch_goblin.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing War-Torch Goblin source: ${candidate}" >&2
  exit 1
fi
echo "War-Torch Goblin module present; isolated Harbor scoring follows submission."
