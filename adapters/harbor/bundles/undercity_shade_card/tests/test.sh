#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/undercity_shade.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Undercity Shade source: ${candidate}" >&2
  exit 1
fi

echo "Undercity Shade source present; isolated Harbor scoring follows submission."
