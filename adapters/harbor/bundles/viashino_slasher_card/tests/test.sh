#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/viashino_slasher.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Viashino Slasher source: ${candidate}" >&2
  exit 1
fi
echo "Viashino Slasher source present; isolated Harbor scoring follows submission."
