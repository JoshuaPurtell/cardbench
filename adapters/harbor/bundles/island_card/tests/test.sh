#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/island.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Island source: ${candidate}" >&2
  exit 1
fi

echo "Island source present; isolated Harbor scoring follows submission."
