#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/birds_of_paradise.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Birds of Paradise source: ${candidate}" >&2
  exit 1
fi
echo "Birds of Paradise module present; isolated Harbor scoring follows submission."
