#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/temple_garden.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Temple Garden source: ${candidate}" >&2
  exit 1
fi

echo "Temple Garden source present; isolated Harbor scoring follows submission."
