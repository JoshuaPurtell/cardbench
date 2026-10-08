#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sabertooth_alley_cat.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Sabertooth Alley Cat candidate source: ${candidate}" >&2
  exit 1
fi
echo "Sabertooth Alley Cat source present; isolated Harbor scoring follows submission."
