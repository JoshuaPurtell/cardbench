#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/mountain.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Mountain source: ${candidate}" >&2
  exit 1
fi

echo "Mountain source present; isolated Harbor scoring follows submission."
