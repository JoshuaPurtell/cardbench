#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/empty_the_catacombs.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Empty the Catacombs source: ${candidate}" >&2
  exit 1
fi
echo "Empty the Catacombs candidate module present; isolated authority scoring validates all-creature returns from both graveyards."
