#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/smash.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Smash source: ${candidate}" >&2
  exit 1
fi
echo "Smash module present; isolated Harbor scoring follows submission."
