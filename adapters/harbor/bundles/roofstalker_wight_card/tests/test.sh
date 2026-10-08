#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/roofstalker_wight.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Roofstalker Wight source: ${candidate}" >&2
  exit 1
fi

echo "Roofstalker Wight source present; isolated Harbor scoring follows submission."
