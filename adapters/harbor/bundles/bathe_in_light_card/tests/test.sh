#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/bathe_in_light.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Bathe in Light source: ${candidate}" >&2
  exit 1
fi
echo "Bathe in Light module present; isolated Harbor scoring follows submission."
