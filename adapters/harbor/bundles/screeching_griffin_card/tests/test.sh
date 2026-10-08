#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/screeching_griffin.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Screeching Griffin source: ${candidate}" >&2
  exit 1
fi

echo "Screeching Griffin source present; isolated Harbor scoring follows submission."
