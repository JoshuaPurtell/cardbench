#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/rain_of_embers.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Rain of Embers source: ${candidate}" >&2
  exit 1
fi

echo "Rain of Embers module present; isolated Harbor scoring follows submission."
