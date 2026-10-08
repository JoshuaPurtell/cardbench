#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Peel from Reality candidate source: ${candidate}" >&2
  exit 1
fi
echo "Peel from Reality source present; isolated Harbor scoring follows submission."
