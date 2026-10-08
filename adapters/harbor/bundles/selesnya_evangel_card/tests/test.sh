#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_evangel.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Selesnya Evangel source: ${candidate}" >&2
  exit 1
fi

echo "Selesnya Evangel source present; isolated Harbor scoring follows submission."
