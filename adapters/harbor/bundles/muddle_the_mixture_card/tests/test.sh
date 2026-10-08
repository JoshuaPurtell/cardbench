#!/usr/bin/env bash
set -euo pipefail

candidate="${CARDBENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/muddle_the_mixture.rs}"
if [[ ! -f "$candidate" ]]; then
  echo "missing Muddle the Mixture source: ${candidate}" >&2
  exit 1
fi
echo "Muddle the Mixture candidate module present; isolated authority scoring validates its behavior."
