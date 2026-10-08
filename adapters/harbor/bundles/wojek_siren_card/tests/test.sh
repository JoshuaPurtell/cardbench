#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Wojek Siren source: ${candidate}" >&2
  exit 1
fi
echo "Wojek Siren module present; isolated Harbor scoring follows submission."
