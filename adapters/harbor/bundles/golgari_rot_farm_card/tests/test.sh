#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rot_farm.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Golgari Rot Farm candidate source: ${candidate}" >&2
  exit 1
fi
echo "Golgari Rot Farm source present; isolated Harbor scoring follows submission."
