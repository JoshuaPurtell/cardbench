#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_garrison.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Boros Garrison candidate source: ${candidate}" >&2
  exit 1
fi
echo "Boros Garrison source present; isolated Harbor scoring follows submission."
