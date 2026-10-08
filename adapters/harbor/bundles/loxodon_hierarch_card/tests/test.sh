#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Loxodon Hierarch source: ${candidate}" >&2
  exit 1
fi

echo "Loxodon Hierarch source present; isolated Harbor scoring follows submission."
