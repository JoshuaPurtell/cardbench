#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/snapping_drake.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Snapping Drake source: ${candidate}" >&2
  exit 1
fi

echo "Snapping Drake source present; isolated Harbor scoring follows submission."
