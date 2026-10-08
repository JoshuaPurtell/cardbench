#!/usr/bin/env bash
set -euo pipefail
candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/dream_leash.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Dream Leash source: ${candidate}" >&2
  exit 1
fi
echo "Dream Leash module present; isolated Harbor scoring follows submission."
