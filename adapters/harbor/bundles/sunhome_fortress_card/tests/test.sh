#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/sunhome_fortress_of_the_legion.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Sunhome source: ${candidate}" >&2
  exit 1
fi

echo "Sunhome source present; isolated Harbor scoring follows submission."
