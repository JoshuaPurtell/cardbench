#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/oathsworn_giant.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Oathsworn Giant source: ${candidate}" >&2
  exit 1
fi

echo "Oathsworn Giant source present; isolated Harbor scoring follows submission."
