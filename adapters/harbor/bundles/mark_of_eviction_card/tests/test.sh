#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/mark_of_eviction.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Mark of Eviction candidate source: ${candidate}" >&2
  exit 1
fi
echo "Mark of Eviction source present; isolated Harbor scoring follows submission."
