#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/telling_time.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Telling Time source: ${candidate}" >&2
  exit 1
fi
echo "Telling Time module present; isolated Harbor scoring follows submission."
