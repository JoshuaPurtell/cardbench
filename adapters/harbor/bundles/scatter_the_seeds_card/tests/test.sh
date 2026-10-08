#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/scatter_the_seeds.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Scatter the Seeds source: ${candidate}" >&2
  exit 1
fi

echo "Scatter the Seeds source present; isolated Harbor scoring follows submission."
