#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/watery_grave.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Watery Grave source: ${candidate}" >&2
  exit 1
fi

echo "Watery Grave source present; isolated Harbor scoring follows submission."
