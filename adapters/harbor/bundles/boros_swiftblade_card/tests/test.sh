#!/usr/bin/env bash
set -euo pipefail

candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_swiftblade.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Boros Swiftblade source: ${candidate}" >&2
  exit 1
fi

echo "Boros Swiftblade source present; isolated Harbor scoring follows submission."
