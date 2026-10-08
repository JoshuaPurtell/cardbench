#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/convolute.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Convolute source: ${candidate}" >&2
  exit 1
fi
echo "Convolute module present; isolated Harbor scoring follows submission."
