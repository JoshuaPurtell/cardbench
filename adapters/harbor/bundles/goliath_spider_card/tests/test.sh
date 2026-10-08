#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/goliath_spider.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Goliath Spider source: ${candidate}" >&2
  exit 1
fi

echo "Goliath Spider source present; isolated Harbor scoring follows submission."
