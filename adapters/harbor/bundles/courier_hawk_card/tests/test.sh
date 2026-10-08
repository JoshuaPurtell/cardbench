#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/courier_hawk.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Courier Hawk source: ${candidate}" >&2
  exit 1
fi

echo "Courier Hawk source present; isolated Harbor scoring follows submission."
