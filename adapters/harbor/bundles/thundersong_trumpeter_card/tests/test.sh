#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/thundersong_trumpeter.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Thundersong Trumpeter source: ${candidate}" >&2
  exit 1
fi

echo "Thundersong Trumpeter source present; isolated Harbor scoring follows submission."
