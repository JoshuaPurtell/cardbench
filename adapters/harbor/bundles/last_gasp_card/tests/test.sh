#!/usr/bin/env bash
set -euo pipefail
candidate="${CARD_BENCH_CANDIDATE:-varieties/magic/sets/ravnica_city_of_guilds/src/cards/last_gasp.rs}"
if [[ ! -f "${candidate}" ]]; then
  echo "missing Last Gasp source: ${candidate}" >&2
  exit 1
fi
echo "Last Gasp source present; isolated Harbor scoring follows submission."
