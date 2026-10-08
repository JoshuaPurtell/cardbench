#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/coalhauler_swine.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Coalhauler Swine candidate module: ${candidate}" >&2
  exit 1
fi

echo "Coalhauler Swine module present; isolated Harbor scoring follows submission."
